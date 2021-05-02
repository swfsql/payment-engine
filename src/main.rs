use tracing::info;

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
    let inputs = payment_engine::read_input_file(&path)?;
    let clients = payment_engine::run(inputs.into_iter());
    payment_engine::write_output(clients.values().cloned(), std::io::stdout())?;

    info!("Execution finished");
    Ok(())
}
