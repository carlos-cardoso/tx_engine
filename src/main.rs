use std::{env, io, path::Path};
use tracing::info;
use tx_engine::{
    csv_input::read_transactions_from_csv, model::Clients, setup_tracing_logs, spawn_writer_thread,
};

fn main() -> io::Result<()> {
    setup_tracing_logs(); // initialize logging to stderr
    info!("Starting the transactions processing application...");

    let mut args = env::args();

    // load input csv
    info!("Loading input csv...");
    let file_path = args.nth(1).expect("No command line argument was provided");
    let file_path = Path::new(&file_path);
    let transactions_iter = read_transactions_from_csv(file_path).expect("failed to load the csv");

    let (tx, rx) = std::sync::mpsc::channel();
    let thread_id = spawn_writer_thread(io::stdout(), rx);

    // apply the transactions
    info!("Applying transactions...");
    let mut clients = Clients::new(tx);
    clients.load_transactions(transactions_iter); //will early write accounts that become locked

    // output to stdout
    info!("Writing remaining clients to stdout...");
    clients // write the remaining (non locked) clients to stdout
        .send_to_output(tx_engine::model::OutputMode::SkipLocked)
        .expect("failed to write to output");

    thread_id.join().expect("failed to join writer thread");
    info!("Finished processing transactions");
    Ok(())
}
