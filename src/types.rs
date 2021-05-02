pub mod client;
pub mod tx;

pub use client::{Client, Clients};
use derive_more as dm;
use rust_decimal as dec;
use serde::{Deserialize, Serialize};
use thiserror::Error;
pub use tx::{ExternalTx, OrderedTxs, Tx, TxId};

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
pub struct ClientId(u16);

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
    dm::From,
    dm::Into,
    Serialize,
    Deserialize,
)]
#[serde(transparent)]
pub struct Amount(dec::Decimal);

#[derive(Clone, Debug, Eq, PartialEq, Error)]
#[error("Rhs is too big on the subtraction {0:?} - {1:?}")]
pub struct RhsSubTooBigError(Amount, Amount);

impl Amount {
    pub fn sufficient_sub(&mut self, rhs: &Self) -> Result<(), RhsSubTooBigError> {
        if *self >= *rhs {
            (*self).0 -= rhs.0;
            Ok(())
        } else {
            Err(RhsSubTooBigError(self.clone(), rhs.clone()))
        }
    }
}
