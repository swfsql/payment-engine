pub mod apply;
pub mod types;

pub use apply::{Apply, Prepare};
use std::collections::HashMap;
use tracing::error;
pub use types::{
    client::{self, Client, Clients},
    tx::{self, ExternalTx, OrderedTxs, TxType},
};

pub fn run(inputs: impl Iterator<Item = ExternalTx>) -> Clients {
    let mut clients: Clients = HashMap::new();
    let mut txs = OrderedTxs::from(vec![]);
    let mut internal_txid = tx::InternalTxId::default();

    for cltx in inputs.into_iter() {
        use apply::Token;

        let id = &cltx.client;
        #[allow(clippy::or_fun_call)]
        let client = clients.entry(id.clone()).or_insert(Client::new(id));

        let (token_client, client) = Token::new(client);
        let (token_txs, client_txs) = Token::new(&mut txs);

        match client
            .try_process_transaction(token_client, &cltx, client_txs, token_txs)
            .map_err(|e| cltx.client_error(e, internal_txid.clone()))
        {
            Ok(()) => match cltx.ty {
                TxType::Deposit | TxType::Withdrawal => {
                    client_txs.push_ordered(tx::Tx::from_external(&cltx, internal_txid.clone()));
                }
                TxType::Dispute | TxType::Resolve | TxType::Chargeback => (),
            },
            Err(e) => {
                error!("{}", e);
            }
        }
        internal_txid.step();
    }

    clients
}

pub fn read_input_file(path: &std::path::Path) -> anyhow::Result<Vec<ExternalTx>> {
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

    let mut reader = csv_reader.from_path(path)?;
    let mut entries = vec![];
    for res in reader.deserialize() {
        let entry: ExternalTx = res?;
        entries.push(entry);
    }
    Ok(entries)
}

pub fn write_output<W: std::io::Write>(
    clients: impl Iterator<Item = Client>,
    wrt: W,
) -> anyhow::Result<()> {
    let mut csv_writer = csv::WriterBuilder::new();
    csv_writer
        .double_quote(false)
        // default
        .delimiter(b',')
        .has_headers(true)
        .flexible(false)
        .terminator(csv::Terminator::CRLF)
        .quote_style(csv::QuoteStyle::Never);
    let mut writer = csv_writer.from_writer(wrt);
    for entry in clients {
        writer.serialize(entry)?;
    }
    writer.flush()?;
    Ok(())
}
