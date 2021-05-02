use crate::types::{client::ClTxError, Amount, ClientId};
use derive_more as dm;
use serde::{Deserialize, Serialize};

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

#[derive(Clone, Debug, Hash, Eq, PartialEq, Ord, PartialOrd, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct ExternalTx {
    #[serde(rename = "type")]
    pub ty: TxType,
    pub client: ClientId,
    pub txid: TxId,
    pub amount: Option<Amount>,
}

#[derive(Clone, Debug, Hash, Eq, PartialEq, Ord, PartialOrd, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct Tx {
    #[serde(rename = "type")]
    pub ty: TxType,
    pub client: ClientId,
    #[serde(rename = "tx")]
    pub txid: TxId,
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
}

impl From<&ExternalTx> for Tx {
    fn from(extx: &ExternalTx) -> Self {
        Tx {
            ty: extx.ty.clone(),
            client: extx.client.clone(),
            txid: extx.txid.clone(),
            amount: extx.amount.clone(),
            disputed: false,
        }
    }
}

// TODO: check precision of amounts before printing
// (they should have at most 4 decimal places)

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

    pub fn get_by_id(&self, tx: &TxId) -> Option<&Tx> {
        self.0
            // assumes the vec is ordered
            .binary_search_by_key(tx, |cltx| cltx.txid.clone())
            .ok()
            .and_then(|index| self.0.get(index))
    }

    pub fn get_mut_by_id(&mut self, tx: &TxId) -> Option<&mut Tx> {
        self.0
            // assumes the vec is ordered
            .binary_search_by_key(tx, |cltx| cltx.txid.clone())
            .ok()
            .and_then(move |index| self.0.get_mut(index))
    }
}
