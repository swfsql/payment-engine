#![feature(str_split_once)]
#![feature(bool_to_option)]

use derive_more as dm;
use rust_decimal as dec;
use serde::{Deserialize, Serialize};
use std::{collections::HashMap, marker::PhantomData};
use thiserror::Error;

type Clients = HashMap<ClientId, Client>;
type Txs = OrderedTxs;

pub trait TryApply
where
    Self: Clone,
{
    fn prepare<'s, E, F>(&'s mut self, cl: F) -> Result<PostApply<'s, Self, E>, E>
    where
        F: FnOnce(&mut Self) -> Result<(), E>,
    {
        let mut next: Self = self.clone();
        cl(&mut next)?;
        Ok(PostApply {
            _self: self,
            next,
            err_ph: PhantomData,
        })
    }

    fn try_chain_apply<Other, E, F1, F2>(
        &mut self,
        cl: F1,
        other: &mut Other,
        cl2: F2,
    ) -> Result<(), E>
    where
        F1: FnOnce(&mut Self) -> Result<(), E>,
        F2: FnOnce(&mut Other) -> Result<(), E>,
        Other: TryApply,
    {
        let mut self_next: Self = self.clone();
        let mut other_next: Other = other.clone();

        cl(&mut self_next)?;
        cl2(&mut other_next)?;

        *self = self_next;
        *other = other_next;

        Ok(())
    }
}

pub struct PostApply<'s, S, E> {
    pub _self: &'s mut S,
    pub next: S,
    err_ph: PhantomData<E>,
}

impl<'s, S, E> PostApply<'s, S, E> {
    pub fn apply(self) -> Result<(), E> {
        *self._self = self.next;
        Ok(())
    }
    pub fn chain_apply<'o, Other>(self, other: PostApply<'o, Other, E>) -> Result<(), E> {
        self.apply()?;
        other.apply()?;
        Ok(())
    }
}

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

impl TryApply for Tx {}

// TODO: check precision of amounts before printing
// (they should have at most 4 decimal places)

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct Client {
    pub id: ClientId,
    pub available: Amount,
    pub held: Amount,
    pub total: Amount,
    pub locked: bool,
}

impl TryApply for Client {}

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
    pub fn try_process_transaction(
        &mut self,
        extx: &ExternalTx,
        previous_txs: &mut Txs,
    ) -> Result<(), ClTxError> {
        use ClTxError::*;
        match &extx.ty {
            TxType::Deposit => {
                let amount = extx.amount.as_ref().ok_or(MissingAmountError)?;
                self.prepare(|next| {
                    next.available += amount.clone();
                    next.total += amount.clone();
                    Ok(())
                })?
                .apply()
            }
            TxType::Withdrawal => {
                let amount = extx.amount.as_ref().ok_or(MissingAmountError)?;
                self.prepare(|next| {
                    next.available.sufficient_sub(amount)?;
                    next.total.sufficient_sub(amount)?;
                    Ok(())
                })?
                .apply()
            }
            TxType::Dispute => {
                if let Some(ref amount) = extx.amount {
                    return Err(ExpectingEmptyAmountError(amount.clone()));
                }

                // extx and disputed_tx would have the same txid information
                let txid = &extx.txid;

                let disputed_tx = previous_txs
                    .get_mut_by_id(&extx.txid)
                    .ok_or_else(|| DisputationOnANotFoundTxIdError(txid.clone()))?;
                if TxType::Deposit != disputed_tx.ty {
                    return Err(DisputationOnNonDepositError(txid.clone()));
                }
                if disputed_tx.disputed {
                    return Err(DisputationOnAlreadyDisputedTxError(txid.clone()));
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
                })?;

                let p2 = disputed_tx.prepare(|next| {
                    next.disputed = true;
                    Ok(())
                })?;

                p1.chain_apply(p2)
            }
            TxType::Resolve => Ok(()),
            TxType::Chargeback => Ok(()),
        }
    }
}

fn main() {
    let client_txs_input = read_client_tx();

    let mut clients: Clients = HashMap::new();
    let mut client_txs = OrderedTxs::from(vec![]);

    for cltx in &client_txs_input {
        let id = &cltx.client;
        let client = clients.entry(id.clone()).or_insert(Client {
            id: id.clone(),
            ..Client::default()
        });
        client
            .try_process_transaction(cltx, &mut client_txs)
            .unwrap();

        // todo: check if tx was ok, then..
        client_txs.push_ordered(cltx.into());
    }

    let mut csv_writer = csv::WriterBuilder::new();
    csv_writer
        .double_quote(false)
        // default
        .delimiter(b',')
        .has_headers(true)
        .flexible(false)
        .terminator(csv::Terminator::CRLF)
        .quote_style(csv::QuoteStyle::Never);
    let mut writer = csv_writer.from_writer(std::io::stdout());
    for entry in client_txs_input {
        writer.serialize(entry).unwrap();
    }
    writer.flush().unwrap();

    println!("finished.");
}

fn read_client_tx() -> Vec<ExternalTx> {
    let reader_path = "tests/1.csv";

    let mut csv_reader = csv::ReaderBuilder::new();
    csv_reader
        .trim(csv::Trim::All)
        .double_quote(false)
        .quoting(false)
        // default
        .delimiter(b',')
        .has_headers(true)
        .flexible(false)
        .terminator(csv::Terminator::CRLF)
        .comment(None);

    let mut reader = csv_reader.from_path(reader_path).unwrap();
    let mut entries = vec![];
    for res in reader.deserialize() {
        let entry: ExternalTx = res.unwrap();
        println!("{:?}", &entry);
        entries.push(entry);
    }
    entries
}
