use crate::{
    apply::Token,
    types::{
        tx::{self, TxType, Txs},
        Amount, ClientId, ExternalTx, RhsSubTooBigError, TxId,
    },
};
use crate::{Apply, Prepare};
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
    #[error("Incoming tx indicates a tx of another client. Incoming tx client: {0:?}, indicated tx client: {1:?}")]
    DifferentClientError(ClientId, ClientId),
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
            Err(ClTxError::DifferentClientError(
                self.id.clone(),
                tx.client.clone(),
            ))
        }
    }
    pub fn try_process_transaction<'c, 't>(
        &'c mut self,
        token_client: Token<'c, Client>,
        extx: &ExternalTx,
        previous_txs: &mut Txs,
        token_txs: Token<'t, Txs>,
    ) -> Result<(), ClTxError> {
        use ClTxError::*;
        match &extx.ty {
            TxType::Deposit => {
                let amount = extx.amount.as_ref().ok_or(MissingAmountError)?;
                self.prepare(|next| {
                    next.available += amount.clone();
                    next.total += amount.clone();
                    Ok(())
                })
                .apply(token_client)
            }
            TxType::Withdrawal => {
                let amount = extx.amount.as_ref().ok_or(MissingAmountError)?;
                if self.locked {
                    return Err(LockedClientError);
                }
                self.prepare(|next| {
                    next.available.sufficient_sub(amount)?;
                    next.total.sufficient_sub(amount)?;
                    Ok(())
                })
                .apply(token_client)
            }
            TxType::Dispute => {
                if let Some(ref amount) = extx.amount {
                    return Err(ExpectingEmptyAmountError(amount.clone()));
                }

                // extx and disputing_tx would have the same txid information
                let txid = &extx.txid;

                let (token_tx, disputing_tx) = previous_txs
                    .get_mut(&extx.txid, token_txs)
                    .ok_or_else(|| DisputationOnANotFoundTxIdError(txid.clone()))?;
                if TxType::Deposit != disputing_tx.ty {
                    return Err(DisputationOnNonDepositError(txid.clone()));
                }
                self.check_client_id(disputing_tx)?;

                let amount = disputing_tx
                    .amount
                    .as_ref()
                    .ok_or(MissingAmountError)?
                    .clone();

                let p1 = self.prepare(|next| {
                    next.available.sufficient_sub(&amount)?;
                    next.held += amount.clone();
                    Ok(())
                });

                let p2 = disputing_tx.prepare(|next| {
                    next.set_disputed()?;
                    Ok(())
                });

                p1.chain(p2).apply(token_client.then(token_tx))
            }
            TxType::Resolve => {
                if let Some(ref amount) = extx.amount {
                    return Err(ExpectingEmptyAmountError(amount.clone()));
                }

                // extx and resolving_tx would have the same txid information
                let txid = &extx.txid;

                let (token_tx, resolving_tx) = previous_txs
                    .get_mut(&extx.txid, token_txs)
                    .ok_or_else(|| ResolvingOnANotFoundTxIdError(txid.clone()))?;
                if TxType::Deposit != resolving_tx.ty {
                    return Err(ResolvingOnNonDepositError(txid.clone()));
                }
                self.check_client_id(resolving_tx)?;

                let amount = resolving_tx
                    .amount
                    .as_ref()
                    .ok_or(MissingAmountError)?
                    .clone();

                let p1 = self.prepare(|next| {
                    next.held.sufficient_sub(&amount)?;
                    next.available += amount.clone();
                    Ok(())
                });

                let p2 = resolving_tx.prepare(|next| {
                    next.unset_disputed()?;
                    Ok(())
                });

                p1.chain(p2).apply(token_client.then(token_tx))
            }
            TxType::Chargeback => {
                if let Some(ref amount) = extx.amount {
                    return Err(ExpectingEmptyAmountError(amount.clone()));
                }

                // extx and chargeback_tx would have the same txid information
                let txid = &extx.txid;

                let chargeback_tx = previous_txs
                    .get(&extx.txid)
                    .ok_or_else(|| ChargebackOnANotFoundTxIdError(txid.clone()))?;
                if TxType::Deposit != chargeback_tx.ty {
                    return Err(ChargebackOnNonDepositError(txid.clone()));
                }
                if !chargeback_tx.is_disputed() {
                    return Err(ChargebackOnNonDisputedTxError(txid.clone()));
                }
                self.check_client_id(chargeback_tx)?;

                let amount = chargeback_tx
                    .amount
                    .as_ref()
                    .ok_or(MissingAmountError)?
                    .clone();

                self.prepare(|next| {
                    next.held.sufficient_sub(&amount)?;
                    next.total.sufficient_sub(&amount)?;
                    next.locked = true;
                    Ok(())
                })
                .apply(token_client)
            }
        }
    }
}
