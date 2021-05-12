use crate::{apply::ConsumedToken, Apply};
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
    ) -> Result<ConsumedToken<'t, (Client, Txs)>, ClTxError> {
        use ClTxError::*;
        match &extx.ty {
            TxType::Deposit => {
                let amount = extx.amount.as_ref().ok_or(MissingAmountError)?;
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
                let amount = extx.amount.as_ref().ok_or(MissingAmountError)?;
                if client.as_ref().locked {
                    return Err(LockedClientError);
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
                    return Err(ExpectingEmptyAmountError(amount.clone()));
                }

                // extx and disputing_tx would have the same txid information
                let txid = &extx.txid;

                let (tx_upper, disputing_tx) = match previous_txs.get_mut(&extx.txid) {
                    Ok((upper, tx)) => (upper, tx),
                    Err(_previous_txs) => {
                        return Err(DisputationOnANotFoundTxIdError(txid.clone()))
                    }
                };
                if TxType::Deposit != disputing_tx.as_ref().ty {
                    return Err(DisputationOnNonDepositError(txid.clone()));
                };

                client.as_ref().check_client_id(disputing_tx.as_ref())?;

                let amount = disputing_tx
                    .as_ref()
                    .amount
                    .as_ref()
                    .ok_or(MissingAmountError)?
                    .clone();

                let client = client.prepare::<_, ClTxError>(|next: &mut Client| {
                    next.available.sufficient_sub(&amount)?;
                    next.held += amount.clone();
                    Ok(())
                });

                let disputing_tx = disputing_tx.prepare::<_, ClTxError>(|next: &mut tx::Tx| {
                    next.set_disputed()?;
                    Ok(())
                });

                let (client, tx) = client.chain(disputing_tx).apply()?.split2();
                let txs = tx_upper.consume(tx);
                Ok(client.then(txs))
            }
            TxType::Resolve => {
                if let Some(ref amount) = extx.amount {
                    return Err(ExpectingEmptyAmountError(amount.clone()));
                }

                // extx and resolving_tx would have the same txid information
                let txid = &extx.txid;

                let (tx_upper, resolving_tx) = match previous_txs.get_mut(&extx.txid) {
                    Ok((upper, tx)) => (upper, tx),
                    Err(_previous_txs) => return Err(ResolvingOnNonDepositError(txid.clone())),
                };

                client.as_ref().check_client_id(resolving_tx.as_ref())?;

                let amount = resolving_tx
                    .as_ref()
                    .amount
                    .as_ref()
                    .ok_or(MissingAmountError)?
                    .clone();

                let client = client.prepare::<_, ClTxError>(|next: &mut Client| {
                    next.held.sufficient_sub(&amount)?;
                    next.available += amount.clone();
                    Ok(())
                });

                let resolving_tx = resolving_tx.prepare::<_, ClTxError>(|next: &mut tx::Tx| {
                    next.unset_disputed()?;
                    Ok(())
                });

                let (client, tx) = client.chain(resolving_tx).apply()?.split2();
                let txs = tx_upper.consume(tx);
                Ok(client.then(txs))
            }
            TxType::Chargeback => {
                if let Some(ref amount) = extx.amount {
                    return Err(ExpectingEmptyAmountError(amount.clone()));
                }

                // extx and chargeback_tx would have the same txid information
                let txid = &extx.txid;

                let chargeback_tx = previous_txs
                    .as_ref()
                    .get(&extx.txid)
                    .ok_or_else(|| ChargebackOnANotFoundTxIdError(txid.clone()))?;

                if TxType::Deposit != chargeback_tx.ty {
                    return Err(ChargebackOnNonDepositError(txid.clone()));
                };
                if !chargeback_tx.is_disputed() {
                    return Err(ChargebackOnNonDisputedTxError(txid.clone()));
                }

                client.as_ref().check_client_id(chargeback_tx)?;

                let amount = chargeback_tx
                    .amount
                    .as_ref()
                    .ok_or(MissingAmountError)?
                    .clone();

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
