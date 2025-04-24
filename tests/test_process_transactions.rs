use std::{
    collections::HashMap,
    io::{Stdout, Write},
    path::Path,
    process::Output,
};

use rust_decimal::{Decimal, dec};
use serde::Serialize;
use tx_engine::{
    csv_input::read_transactions_from_csv,
    model::{Account, ClientId, Clients},
};

#[test]
fn deposits_withdrawals() {
    let transactions_iter = read_transactions_from_csv(Path::new("data/input_example.csv"))
        .expect("failed to load the csv");
    let mut clients = Clients::default();
    clients
        .load_transactions(transactions_iter)
        .expect("invalid transactions");

    let expected_client_1 = Account::new(dec!(1.5), dec!(0.0), false);
    let expected_client_2 = Account::new(dec!(2.0), dec!(0.0), false);
    assert_eq!(clients.0[&ClientId(1)].account(), expected_client_1);
    assert_eq!(clients.0[&ClientId(2)].account(), expected_client_2);
}

#[test]
fn dispute() {
    let transactions_iter = read_transactions_from_csv(Path::new("data/input_example_dispute.csv"))
        .expect("failed to load the csv");
    let mut clients = Clients::default();
    clients
        .load_transactions(transactions_iter)
        .expect("invalid transactions");

    let expected_client_1 = Account::new(dec!(1.0), dec!(0.5), false);
    let expected_client_2 = Account::new(dec!(1.0), dec!(1.0), false);
    assert_eq!(clients.0[&ClientId(1)].account(), expected_client_1);
    assert_eq!(clients.0[&ClientId(2)].account(), expected_client_2);
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
    let transactions_iter = read_transactions_from_csv(Path::new("data/input_example.csv"))
        .expect("failed to load the csv");
    let mut clients = Clients::default();
    clients
        .load_transactions(transactions_iter)
        .expect("invalid transactions");

    // create a Vec to write to (instead of stdout)
    let mut out: Vec<u8> = Vec::new();
    clients.write(&mut out);

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
