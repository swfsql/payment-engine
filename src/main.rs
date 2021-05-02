use payment_engine::{apply::Token, tx, Client, Clients, ExternalTx, OrderedTxs};
use std::collections::HashMap;
use tracing::{error, info};

fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_writer(std::io::stderr)
        .init();
    tracing::info!("Execution started");

    let args: Vec<String> = std::env::args().collect();
    if args.len() != 2 {
        anyhow::bail!("Usage: cargo run -- transactions.csv");
    }

    let path = std::path::PathBuf::from(args[1].trim());

    // TODO: make streamable
    let client_txs_input = read_client_tx(&path)?;

    let mut clients: Clients = HashMap::new();
    let mut client_txs = OrderedTxs::from(vec![]);
    let mut internal_txid = tx::InternalTxId::default();

    for cltx in &client_txs_input {
        let id = &cltx.client;
        #[allow(clippy::or_fun_call)]
        let client = clients.entry(id.clone()).or_insert(Client::new(id));

        let (token_client, client) = Token::new(client);
        let (token_txs, client_txs) = Token::new(&mut client_txs);

        // TODO: wrap inner error in an outer one,
        // which should log the current txid
        // (or the internal txid for those that doesn't have a meaningful txid)

        match client
            .try_process_transaction(token_client, cltx, client_txs, token_txs)
            .map_err(|e| cltx.client_error(e, internal_txid.clone()))
        {
            Ok(()) => {
                client_txs.push_ordered(tx::Tx::from_external(cltx, internal_txid.clone()));
            }
            Err(e) => {
                error!("{}", e);
            }
        }
        internal_txid.step();
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
    for entry in clients.values() {
        writer.serialize(entry)?;
    }
    writer.flush()?;

    info!("Execution finished");
    Ok(())
}

fn read_client_tx(path: &std::path::Path) -> anyhow::Result<Vec<ExternalTx>> {
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
