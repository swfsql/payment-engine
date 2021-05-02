use payment_engine::{
    apply::Token,
    types::{Client, Clients, ExternalTx, OrderedTxs},
};
use std::collections::HashMap;

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

        let (token_client, client) = Token::new(client);
        let (token_txs, client_txs) = Token::new(&mut client_txs);

        client
            .try_process_transaction(token_client, cltx, client_txs, token_txs)
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
