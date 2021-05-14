use crate::{err, try_on, Apply, TResult};
use crate::{
    types::{
        tx::{self, TxType, Txs},
        Amount, ClientId, ExternalTx, RhsSubTooBigError, TxId,
    },
    TP,
};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use thiserror::Error;

pub type Clients = HashMap<ClientId, Client>;

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct Client {
    #[serde(rename = "client")]
    pub id: ClientId,
    pub available: Amount,
    pub held: Amount,
    pub total: Amount,
    pub locked: bool,
}

#[derive(Debug, Error)]
pub enum ClTxError {
    #[error("Incoming tx is missing the amount field")]
    MissingAmountError,
    #[error(
        "Incoming tx requires a subtraction from insufficient founds. Subtraction is {0:?} - {1:?}"
    )]
    InsufficientFoundsError(Amount, Amount),
    #[error("Incoming tx has the amount field when none was expected. Found: {0:?}")]
    ExpectingEmptyAmountError(Amount),
    //
    #[error("Incoming tx indicates a tx of another client. Incoming tx client: {incoming:?}, indicated tx client: {stored:?}")]
    DifferentClientError {
        incoming: ClientId,
        stored: ClientId,
    },
    #[error("The client is locked")]
    LockedClientError,
    //
    #[error("Incoming tx indicates a non-existent tx {0:?}")]
    DisputationOnANotFoundTxIdError(TxId),
    #[error("Incoming tx indicates a non-deposit tx {0:?}")]
    DisputationOnNonDepositError(TxId),
    #[error("Incoming tx indicates an already disputed tx {0:?}")]
    DisputationOnAlreadyDisputedTxError(TxId),
    //
    #[error("Incoming tx indicates a non-existent tx {0:?}")]
    ResolvingOnANotFoundTxIdError(TxId),
    #[error("Incoming tx indicates a non-deposit tx {0:?}")]
    ResolvingOnNonDepositError(TxId),
    #[error("Incoming tx indicates an non-disputed tx {0:?}")]
    ResolvingOnNonDisputedTxError(TxId),
    //
    #[error("Incoming tx indicates a non-existent tx {0:?}")]
    ChargebackOnANotFoundTxIdError(TxId),
    #[error("Incoming tx indicates a non-deposit tx {0:?}")]
    ChargebackOnNonDepositError(TxId),
    #[error("Incoming tx indicates an non-disputed tx {0:?}")]
    ChargebackOnNonDisputedTxError(TxId),
}

impl From<RhsSubTooBigError> for ClTxError {
    fn from(e: RhsSubTooBigError) -> Self {
        ClTxError::InsufficientFoundsError(e.0, e.1)
    }
}

impl Client {
    pub fn new(id: &ClientId) -> Self {
        Client {
            id: id.clone(),
            ..Self::default()
        }
    }
    pub fn check_client_id(&self, tx: &tx::Tx) -> Result<(), ClTxError> {
        if self.id == tx.client {
            Ok(())
        } else {
            Err(ClTxError::DifferentClientError {
                incoming: self.id.clone(),
                stored: tx.client.clone(),
            })
        }
    }

    pub fn try_process_transaction<'t>(
        client: TP<'t, Client>,
        extx: &'t ExternalTx,
        previous_txs: TP<'t, Txs>,
    ) -> TResult<'t, (Client, Txs), ClTxError> {
        use ClTxError::*;
        match &extx.ty {
            TxType::Deposit => {
                let amount = extx.amount.as_ref().ok_or(MissingAmountError);
                let amount = try_on!(amount, client, previous_txs);
                client
                    .prepare(move |next: &mut Client| {
                        next.available += amount.clone();
                        next.total += amount.clone();
                        Ok(())
                    })
                    .chain(previous_txs.skip())
                    .apply()
            }
            TxType::Withdrawal => {
                let amount = extx.amount.as_ref().ok_or(MissingAmountError);
                let amount = try_on!(amount, client, previous_txs);
                if client.as_ref().locked {
                    let err = LockedClientError;
                    return err!(err, client, previous_txs);
                }

                let client = client.prepare(move |next: &mut Client| {
                    next.available.sufficient_sub(amount)?;
                    next.total.sufficient_sub(amount)?;
                    Ok(())
                });

                client.chain(previous_txs.skip()).apply()
            }
            TxType::Dispute => {
                if let Some(ref amount) = extx.amount {
                    let err = ExpectingEmptyAmountError(amount.clone());
                    return err!(err, client, previous_txs);
                }

                // extx and disputing_tx would have the same txid information
                let txid = &extx.txid;

                let (tx_upper, disputing_tx) = match previous_txs.get_mut(&extx.txid) {
                    Ok(ok) => ok,
                    Err(previous_txs) => {
                        let err = DisputationOnANotFoundTxIdError(txid.clone());
                        return err!(err, client, previous_txs);
                    }
                };

                if TxType::Deposit != disputing_tx.as_ref().ty {
                    let err = DisputationOnNonDepositError(txid.clone());
                    return err!(err, client, tx_upper.returned(disputing_tx));
                };

                let check = client.as_ref().check_client_id(disputing_tx.as_ref());
                try_on!(check, client, tx_upper.returned(disputing_tx));

                let amount = disputing_tx
                    .as_ref()
                    .amount
                    .as_ref()
                    .ok_or(MissingAmountError);
                let amount = try_on!(amount, client, tx_upper.returned(disputing_tx)).clone();

                let client = client.prepare::<_, ClTxError>(|next: &mut Client| {
                    next.available.sufficient_sub(&amount)?;
                    next.held += amount.clone();
                    Ok(())
                });

                let disputing_tx = disputing_tx.prepare::<_, ClTxError>(|next: &mut tx::Tx| {
                    next.set_disputed()?;
                    Ok(())
                });

                match client.chain(disputing_tx).apply() {
                    Ok(tokens) => {
                        let (client, tx) = tokens.split2();
                        Ok(client.then(tx_upper.consume(tx)))
                    }
                    Err((e, tokens)) => {
                        let (client, tx) = tokens.split2();
                        err!(e, client, tx_upper.returned(tx))
                    }
                }
            }
            TxType::Resolve => {
                if let Some(ref amount) = extx.amount {
                    let err = ExpectingEmptyAmountError(amount.clone());
                    return err!(err, client, previous_txs);
                }

                // extx and resolving_tx would have the same txid information
                let txid = &extx.txid;

                let (tx_upper, resolving_tx) = match previous_txs.get_mut(&extx.txid) {
                    Ok(ok) => ok,
                    Err(previous_txs) => {
                        let err = ResolvingOnNonDepositError(txid.clone());
                        return err!(err, client, previous_txs);
                    }
                };

                let check = client.as_ref().check_client_id(resolving_tx.as_ref());
                try_on!(check, client, tx_upper.returned(resolving_tx));

                let amount = resolving_tx
                    .as_ref()
                    .amount
                    .as_ref()
                    .ok_or(MissingAmountError);
                let amount = try_on!(amount, client, tx_upper.returned(resolving_tx)).clone();

                let client = client.prepare::<_, ClTxError>(|next: &mut Client| {
                    next.held.sufficient_sub(&amount)?;
                    next.available += amount.clone();
                    Ok(())
                });

                let resolving_tx = resolving_tx.prepare::<_, ClTxError>(|next: &mut tx::Tx| {
                    next.unset_disputed()?;
                    Ok(())
                });

                match client.chain(resolving_tx).apply() {
                    Ok(tokens) => {
                        let (client, tx) = tokens.split2();
                        Ok(client.then(tx_upper.consume(tx)))
                    }
                    Err((e, tokens)) => {
                        let (client, tx) = tokens.split2();
                        err!(e, client, tx_upper.returned(tx))
                    }
                }
            }
            TxType::Chargeback => {
                if let Some(ref amount) = extx.amount {
                    let err = ExpectingEmptyAmountError(amount.clone());
                    return err!(err, client, previous_txs);
                };

                // extx and chargeback_tx would have the same txid information
                let txid = &extx.txid;

                let chargeback_tx = previous_txs
                    .as_ref()
                    .get(&extx.txid)
                    .ok_or_else(|| ChargebackOnANotFoundTxIdError(txid.clone()));
                let chargeback_tx = try_on!(chargeback_tx, client, previous_txs);

                if TxType::Deposit != chargeback_tx.ty {
                    let err = ChargebackOnNonDepositError(txid.clone());
                    return err!(err, client, previous_txs);
                };
                if !chargeback_tx.is_disputed() {
                    let err = ChargebackOnNonDisputedTxError(txid.clone());
                    return err!(err, client, previous_txs);
                };

                let check = client.as_ref().check_client_id(chargeback_tx);
                try_on!(check, client, previous_txs);

                let amount = chargeback_tx.amount.as_ref().ok_or(MissingAmountError);
                let amount = try_on!(amount, client, previous_txs).clone();

                let client = client.prepare::<_, ClTxError>(move |next: &mut Client| {
                    next.held.sufficient_sub(&amount)?;
                    next.total.sufficient_sub(&amount)?;
                    next.locked = true;
                    Ok(())
                });

                client.chain(previous_txs.skip()).apply()
            }
        }
    }
}
