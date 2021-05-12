use crate::{
    apply::DowngradedTokenProtected,
    types::{client::ClTxError, Amount, ClientId},
    TP,
};
use derive_more as dm;
use serde::{Deserialize, Serialize};
use thiserror::Error;

pub type Txs = OrderedTxs;

#[derive(Clone, Debug, Hash, Eq, PartialEq, Ord, PartialOrd, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TxType {
    Deposit,
    Withdrawal,
    Dispute,
    Resolve,
    Chargeback,
}

#[derive(
    Clone,
    Debug,
    Default,
    Hash,
    Eq,
    PartialEq,
    Ord,
    PartialOrd,
    dm::Add,
    dm::AddAssign,
    dm::Sub,
    dm::SubAssign,
    dm::From,
    dm::Into,
    Serialize,
    Deserialize,
)]
#[serde(transparent)]
pub struct TxId(u32);

/// `InternalTxId` is used because some transactions, such as the dispute type,
/// does refer to other transactions but they don't have an id themselves,
/// which could prevent a more complete logging of those type of transactions.
///
/// So the `InternalTxId` is incremented by a step for each incoming transaction,
/// even if it was ignored.
#[derive(
    Clone,
    Debug,
    Default,
    Hash,
    Eq,
    PartialEq,
    Ord,
    PartialOrd,
    dm::Add,
    dm::AddAssign,
    dm::Sub,
    dm::SubAssign,
    dm::From,
    dm::Into,
    Serialize,
    Deserialize,
)]
#[serde(transparent)]
pub struct InternalTxId(u32);
impl InternalTxId {
    pub fn step(&mut self) {
        self.0 += 1;
    }
}

/// The `ExternalTx` are consumed by the clients, but they are not stored
/// in the programs internal state (HashMaps).  
/// Instead, the `Tx` are, because they have extra field that can be change,
/// such as the `disputed` status.
#[derive(Clone, Debug, Hash, Eq, PartialEq, Ord, PartialOrd, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct ExternalTx {
    #[serde(rename = "type")]
    pub ty: TxType,
    pub client: ClientId,
    #[serde(rename = "tx")]
    pub txid: TxId,
    pub amount: Option<Amount>,
}

impl ExternalTx {
    pub fn client_error(&self, error: ClTxError, internal_txid: InternalTxId) -> TxError {
        TxError {
            txid: self.txid.clone(),
            internal_txid,
            error,
        }
    }
}

#[derive(Clone, Debug, Hash, Eq, PartialEq, Ord, PartialOrd)]
pub struct Tx {
    pub ty: TxType,
    pub client: ClientId,
    pub txid: TxId,
    pub internal_txid: InternalTxId,
    pub amount: Option<Amount>,
    disputed: bool,
}

impl Tx {
    pub fn is_disputed(&self) -> bool {
        self.disputed
    }
    pub fn set_disputed(&mut self) -> Result<(), ClTxError> {
        if self.disputed {
            Err(ClTxError::DisputationOnAlreadyDisputedTxError(
                self.txid.clone(),
            ))
        } else {
            self.disputed = true;
            Ok(())
        }
    }
    pub fn unset_disputed(&mut self) -> Result<(), ClTxError> {
        if !self.disputed {
            Err(ClTxError::ResolvingOnNonDisputedTxError(self.txid.clone()))
        } else {
            self.disputed = false;
            Ok(())
        }
    }
    pub fn from_external(external: &ExternalTx, internal_txid: InternalTxId) -> Self {
        Self {
            ty: external.ty.clone(),
            client: external.client.clone(),
            txid: external.txid.clone(),
            internal_txid,
            amount: external.amount.clone(),
            disputed: false,
        }
    }
    pub fn check_client_id(&self, client_id: &ClientId) -> Result<(), ClTxError> {
        if &self.client == client_id {
            Ok(())
        } else {
            Err(ClTxError::DifferentClientError {
                incoming: client_id.clone(),
                stored: self.client.clone(),
            })
        }
    }
}

#[derive(Debug, Error)]
#[error("Incoming tx {txid:?}, internal id {internal_txid:?}. error: {error}")]
pub struct TxError {
    txid: TxId,
    internal_txid: InternalTxId,
    error: ClTxError,
}

#[derive(Clone, Debug, Default, Hash, Eq, PartialEq, Ord, PartialOrd, dm::From, dm::Into)]
pub struct OrderedTxs(Vec<Tx>);

impl OrderedTxs {
    pub fn push_ordered(&mut self, client_tx: Tx) {
        if let Some(last_id) = self.0.last().map(|cltx| &cltx.txid) {
            // a drafty way to guarantee that it's ordered
            assert!(last_id < &client_tx.txid);
        }
        self.0.push(client_tx);
    }

    pub fn get(&self, tx: &TxId) -> Option<&Tx> {
        self.0
            // assumes the vec is ordered
            .binary_search_by_key(tx, |cltx| cltx.txid.clone())
            .ok()
            .and_then(|index| self.0.get(index))
    }
}

impl<'t> TP<'t, OrderedTxs> {
    pub fn get_mut(
        &'t mut self,
        tx: &TxId,
    ) -> Option<DowngradedTokenProtected<'t, '_, OrderedTxs, Tx, TP<'_, Tx>>> {
        let txs = self;
        let index = txs
            .as_ref()
            .0 // assumes the vec is ordered
            .binary_search_by_key(tx, |cltx| cltx.txid.clone())
            .ok()?;

        // safety: the token protection workflow is maintained.
        // 1. no push/remove into/from txs (the container);
        // 2. txs' token is consumed;
        // 3. tx is token protected;
        // 4. (optional) txs' token and tx's tokens are related
        // (for posterior token upgrade).
        let (inner, token) = unsafe { txs.split_mut() };
        let tx = inner
            .0
            // the `get_mut` doesn't directly change the container,
            .get_mut(index)
            // and the inner item is token protected
            .map(TP::new)?;
        // safety: tx is guaranteed to have come from txs
        // ie. this Tx came from the Vec<Tx>.
        //
        // token relation between the container and the item is created,
        // and the container token is also consumed
        let down = unsafe { token.with_downgrade(tx) };
        // let down = unsafe { txs.downgrade(tx) };

        Some(down)
    }
}
