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
