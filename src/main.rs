use std::{env, io, path::Path};
use tx_engine::{csv_input::read_transactions_from_csv, model::Clients};

fn main() -> io::Result<()> {
    let mut args = env::args();

    let file_path = args.nth(1).expect("No command line argument was provided");
    let file_path = Path::new(&file_path);
    let transactions_iter = read_transactions_from_csv(file_path).expect("failed to load the csv");

    let mut clients = Clients::default();
    clients
        .load_transactions(transactions_iter)
        .expect("There are malformed transactions");

    let stdout_handle = io::stdout();
    clients.write(stdout_handle)
}
