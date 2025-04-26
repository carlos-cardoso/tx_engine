use std::{env, io, path::Path};
use tracing::info;
use tx_engine::{csv_input::read_transactions_from_csv, model::Clients, setup_tracing_logs};

fn main() -> io::Result<()> {
    setup_tracing_logs(); // initialize logging
    info!("Starting the transactions processing application...");

    let mut args = env::args();

    // load input csv
    info!("Loading input csv...");
    let file_path = args.nth(1).expect("No command line argument was provided");
    let file_path = Path::new(&file_path);
    let transactions_iter = read_transactions_from_csv(file_path).expect("failed to load the csv");

    // apply the transactions
    info!("Applying transactions...");
    let mut clients = Clients::default();
    clients.load_transactions(transactions_iter);

    // output to stdout
    info!("Writing to stdout...");
    let stdout_handle = io::stdout();
    clients.write(stdout_handle)?;

    info!("Finished processing transactions");
    Ok(())
}
