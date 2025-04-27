use std::{io, path::Path, sync::mpsc};

use rust_decimal::dec;
use tx_engine::{
    csv_input::{read_transactions_from_csv, transactions_from_reader},
    model::{Account, ClientId, Clients, OutputMode},
    spawn_writer_thread,
};

#[test]
fn deposits_withdrawals() {
    let _ = tracing_subscriber::fmt().with_test_writer().try_init();
    let transactions_iter = read_transactions_from_csv(Path::new("data/input_example.csv"))
        .expect("failed to load the csv");
    let (tx, rx) = mpsc::channel();
    let _thread_id = spawn_writer_thread(io::sink(), rx);
    let mut clients = Clients::new(tx);
    clients.load_transactions(transactions_iter);

    let expected_client_1 = Account::new(dec!(1.5), dec!(0.0), false);
    let expected_client_2 = Account::new(dec!(2.0), dec!(0.0), false);
    assert_eq!(clients.accounts[&ClientId(1)], expected_client_1);
    assert_eq!(clients.accounts[&ClientId(2)], expected_client_2);
}

#[test]
fn dispute() {
    let _ = tracing_subscriber::fmt().with_test_writer().try_init();
    let input_reader = r#"
        type, client, tx, amount
        deposit, 1, 1, 1.0
        deposit, 2, 2, 2.0
        deposit, 1, 3, 2.0
        withdrawal, 1, 4, 1.5
        withdrawal, 2, 5, 3.0
        dispute, 2, 5, 
        dispute, 1, 1,"#
        .as_bytes();
    let csv_reader = csv::ReaderBuilder::new()
        .trim(csv::Trim::All) //trim whitespace around fields
        .from_reader(input_reader);
    let transactions_iter = transactions_from_reader(csv_reader);

    let (tx, rx) = mpsc::channel();
    let _thread_id = spawn_writer_thread(io::sink(), rx);
    let mut clients = Clients::new(tx);

    clients.load_transactions(transactions_iter);

    let expected_client_1 = Account::new(dec!(0.5), dec!(1.0), false);
    let expected_client_2 = Account::new(dec!(2.0), dec!(0.0), false);
    assert_eq!(clients.accounts[&ClientId(1)], expected_client_1);
    assert_eq!(clients.accounts[&ClientId(2)], expected_client_2);
}

#[test]
fn resolve() {
    let _ = tracing_subscriber::fmt().with_test_writer().try_init();
    let input_reader = r#"
        type, client, tx, amount
        deposit, 1, 1, 1.0
        deposit, 2, 2, 2.0
        deposit, 1, 3, 2.0
        withdrawal, 1, 4, 1.5
        withdrawal, 2, 5, 3.0
        dispute, 2, 5,
        dispute, 1, 1,
        resolve, 1, 1,"#
        .as_bytes();
    let csv_reader = csv::ReaderBuilder::new()
        .trim(csv::Trim::All) //trim whitespace around fields
        .from_reader(input_reader);
    let transactions_iter = transactions_from_reader(csv_reader);

    let (tx, rx) = mpsc::channel();
    let _thread_id = spawn_writer_thread(io::sink(), rx);
    let mut clients = Clients::new(tx);

    clients.load_transactions(transactions_iter);

    let expected_client_1 = Account::new(dec!(1.5), dec!(0.0), false);
    let expected_client_2 = Account::new(dec!(2.0), dec!(0.0), false);
    assert_eq!(clients.accounts[&ClientId(1)], expected_client_1);
    assert_eq!(clients.accounts[&ClientId(2)], expected_client_2);
}

#[test]
fn chargeback() {
    let _ = tracing_subscriber::fmt().with_test_writer().try_init();
    let input_reader = r#"
        type, client, tx, amount
        deposit, 1, 1, 1.0
        deposit, 2, 2, 2.0
        deposit, 1, 3, 2.0
        withdrawal, 1, 4, 1.5
        withdrawal, 2, 5, 3.0
        dispute, 2, 5,
        dispute, 1, 1,
        chargeback, 1, 1,"#
        .as_bytes();
    let csv_reader = csv::ReaderBuilder::new()
        .trim(csv::Trim::All) //trim whitespace around fields
        .from_reader(input_reader);
    let transactions_iter = transactions_from_reader(csv_reader);

    let (tx, rx) = mpsc::channel();
    let _thread_id = spawn_writer_thread(io::sink(), rx);
    let mut clients = Clients::new(tx);

    clients.load_transactions(transactions_iter);

    let expected_client_1 = Account::new(dec!(0.5), dec!(0.0), true);
    let expected_client_2 = Account::new(dec!(2.0), dec!(0.0), false);
    assert_eq!(clients.accounts[&ClientId(1)], expected_client_1);
    assert_eq!(clients.accounts[&ClientId(2)], expected_client_2);
}

#[test]
//deposit to a locked account should not change anything
fn locked() {
    let _ = tracing_subscriber::fmt().with_test_writer().try_init();
    let input_reader = r#"
        type, client, tx, amount
        deposit, 1, 1, 1.0
        deposit, 2, 2, 2.0
        deposit, 1, 3, 2.0
        withdrawal, 1, 4, 1.5
        withdrawal, 2, 5, 3.0
        dispute, 2, 2,
        dispute, 1, 3,
        deposit, 2, 6, 2.0
        chargeback, 1, 3,
        chargeback, 2, 2,
        deposit, 1, 7, 1.0
        withdrawal, 2, 8, 1.0"#
        .as_bytes();
    let csv_reader = csv::ReaderBuilder::new()
        .trim(csv::Trim::All) //trim whitespace around fields
        .from_reader(input_reader);
    let transactions_iter = transactions_from_reader(csv_reader);

    let (tx, rx) = mpsc::channel();
    let _thread_id = spawn_writer_thread(io::sink(), rx);
    let mut clients = Clients::new(tx);

    clients.load_transactions(transactions_iter);

    let expected_client_1 = Account::new(dec!(-0.5), dec!(0.0), true);
    let expected_client_2 = Account::new(dec!(2.0), dec!(0.0), true);
    assert_eq!(clients.accounts[&ClientId(1)], expected_client_1);
    assert_eq!(clients.accounts[&ClientId(2)], expected_client_2);
}

#[test]
fn bankers_rounding() {
    let client_15 = Account::new(dec!(0.00015), dec!(0.0), false);
    let client_25 = Account::new(dec!(0.00025), dec!(0.0), false);

    let client_35 = Account::new(dec!(0.00035), dec!(0.0), false);
    let client_45 = Account::new(dec!(0.00045), dec!(0.0), false);

    //check that rounding 0.00015 == 0.00025 == 0.0002
    assert_eq!(client_15.available(), client_25.available());
    assert_eq!(client_15.total(), client_25.total());

    //check that rounding 0.00035 == 0.00045 == 0.0003
    assert_eq!(client_35.available(), client_45.available());

    //check that rounding 0.00025 != 0.00035
    assert_ne!(client_25.available(), client_35.available());
    assert_ne!(client_25.total(), client_35.total());
}

#[test]
/// Validate that we print the expected output
fn output() {
    let _ = tracing_subscriber::fmt().with_test_writer().try_init();
    let transactions_iter = read_transactions_from_csv(Path::new("data/input_example.csv"))
        .expect("failed to load the csv");

    let out: Vec<u8> = Vec::new();
    let (tx, rx) = mpsc::channel();
    let thread_id = spawn_writer_thread(out, rx);
    let mut clients = Clients::new(tx);

    clients.load_transactions(transactions_iter);

    // create a Vec to write to (instead of stdout)
    clients
        .send_to_output(OutputMode::SkipLocked)
        .expect("failed to write to output");

    let csv_writer = thread_id.join().expect("error joining thread");
    let out = csv_writer.into_inner().expect("failed to get inner");

    // sort the lines (Since the order of the csv lines is non-deterministic since we use a HashMap internally)
    let output_string = String::from_utf8(out).expect("invalid utf8");
    let mut lines = output_string.lines();

    let header = lines.next().expect("header line is missing"); // the header should not be sorted

    let mut other_lines: Vec<&str> = lines.collect();
    other_lines.sort_unstable();
    let other_lines = other_lines.join("\n");

    //put the header together with the sorted lines again
    let output_string = format!("{header}\n{other_lines}\n");

    let expected =
        "client,available,held,total,locked\n1,1.5,0,1.5,false\n2,2,0,2,false\n".to_string();
    assert_eq!(output_string, expected);
}
