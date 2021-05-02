use crate::{
    apply::Token,
    types::{
        tx::{TxType, Txs},
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
    #[error("Incoming tx indicates a non-existent tx {0:?}")]
    DisputationOnANotFoundTxIdError(TxId),
    #[error("Incoming tx indicates a non-deposit tx {0:?}")]
    DisputationOnNonDepositError(TxId),
    #[error("Incoming tx indicates an already disputed tx {0:?}")]
    DisputationOnAlreadyDisputedTxError(TxId),
}

impl From<RhsSubTooBigError> for ClTxError {
    fn from(e: RhsSubTooBigError) -> Self {
        ClTxError::InsufficientFoundsError(e.0, e.1)
    }
}

impl Client {
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

                // extx and disputed_tx would have the same txid information
                let txid = &extx.txid;

                let (token_tx, disputed_tx) = previous_txs
                    .get_mut(&extx.txid, token_txs)
                    .ok_or_else(|| DisputationOnANotFoundTxIdError(txid.clone()))?;
                if TxType::Deposit != disputed_tx.ty {
                    return Err(DisputationOnNonDepositError(txid.clone()));
                }

                let amount = disputed_tx
                    .amount
                    .as_ref()
                    .ok_or(MissingAmountError)?
                    .clone();

                let p1 = self.prepare(|next| {
                    next.available.sufficient_sub(&amount)?;
                    next.held += amount.clone();
                    Ok(())
                });

                let p2 = disputed_tx.prepare(|next| {
                    next.set_disputed()?;
                    Ok(())
                });

                p1.chain(p2).apply(token_client.then(token_tx))
            }
            TxType::Resolve => todo!(),
            TxType::Chargeback => todo!(),
        }
    }
}
