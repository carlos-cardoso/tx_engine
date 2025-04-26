use std::{
    io,
    sync::mpsc::Receiver,
    thread::{self, JoinHandle},
};

use csv::Writer;
use model::{Account, ClientId, CsvOutputAccount};
use tracing::error;
use tracing_subscriber::EnvFilter;

pub mod csv_input;
pub mod model;

pub fn setup_tracing_logs() {
    tracing_subscriber::fmt()
        .compact()
        .with_file(true) // usefull since this is not a end user application, it's fine to be specific where the error happened
        .with_line_number(true)
        // .with_thread_ids(true) //unecessary for a single threaded application
        .with_target(false)
        .with_writer(std::io::stderr) // write to stderr to not polute the stdout that is meant to be piped to a csv file
        .with_env_filter(EnvFilter::from_default_env()) // use env filter (e.g. RUST_LOG=trace cargo run -- transactions.csv)
        .init();
}

pub fn spawn_writer_thread<W: io::Write + Send + 'static>(
    wtr: W,
    rx: Receiver<(ClientId, Account)>,
) -> JoinHandle<Writer<W>> {
    thread::spawn(move || {
        let mut csv_writer = csv::WriterBuilder::new().from_writer(wtr);
        loop {
            match rx.recv() {
                Ok((client, account)) => {
                    if let Err(err) =
                        csv_writer.serialize(CsvOutputAccount::from((&client, &account)))
                    {
                        error!(%err, %client, ?account, "failed to serialize account");
                    }
                }
                Err(_err) => {
                    //channel was closed indicating nothing else needs to be written
                    csv_writer.flush().expect("failed to flush");
                    break;
                }
            }
        }
        csv_writer
    })
}
