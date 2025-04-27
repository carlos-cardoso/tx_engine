use std::path::Path;
use tx_engine::csv_input::{ConversionError, read_transactions_from_csv, transactions_from_reader};

/// loads the sample csv
#[test]
fn load_valid_csv() {
    let _ = tracing_subscriber::fmt().with_test_writer().try_init();
    let mut transactions_iter = read_transactions_from_csv(Path::new("data/input_example.csv"))
        .expect("failed to load the csv");
    assert!(transactions_iter.all(|t| t.is_ok()));
}

/// the following tests use a in memory mocked csv to test validations of the transactions
#[test]
fn malformed_transaction() {
    let _ = tracing_subscriber::fmt().with_test_writer().try_init();
    //mock csv input
    let input_reader = r#"
        type, client, tx, amount
        deposit, 1, 1, 1.0
        deposit, 1
    "#
    .as_bytes();

    let csv_reader = csv::ReaderBuilder::new()
        .trim(csv::Trim::All) //trim whitespace around fields
        .from_reader(input_reader);
    let mut transactions_iter = transactions_from_reader(csv_reader);
    assert!(transactions_iter.any(|t| t.is_err_and(|e| matches!(e, ConversionError::CsvError(_)))));
}

#[test]
fn invalid_transaction_type() {
    let _ = tracing_subscriber::fmt().with_test_writer().try_init();
    //mock csv input
    let input_reader = r#"
        type, client, tx, amount
        deposit, 1, 1, 1.0
        move, 1, 1, 1.0
    "#
    .as_bytes();

    let csv_reader = csv::ReaderBuilder::new()
        .trim(csv::Trim::All) //trim whitespace around fields
        .from_reader(input_reader);
    let mut transactions_iter = transactions_from_reader(csv_reader);
    assert!(
        transactions_iter
            .any(|t| t.is_err_and(|e| matches!(e, ConversionError::InvalidTransactionType(_))))
    );
}

#[test]
fn missing_amount() {
    let _ = tracing_subscriber::fmt().with_test_writer().try_init();
    //mock csv input
    let input_reader = r#"
        type, client, tx, amount
        deposit, 1, 1, 1.0
        deposit, 1, 1, 
    "#
    .as_bytes();

    let csv_reader = csv::ReaderBuilder::new()
        .trim(csv::Trim::All) //trim whitespace around fields
        .from_reader(input_reader);
    let mut transactions_iter = transactions_from_reader(csv_reader);
    assert!(
        transactions_iter.any(|t| t.is_err_and(|e| matches!(e, ConversionError::MissingAmount(_))))
    );
}

#[test]
fn invalid_client_id() {
    let _ = tracing_subscriber::fmt().with_test_writer().try_init();
    //mock csv input
    let input_reader = r#"
        type, client, tx, amount
        deposit, 1, 1, 1.0
        deposit, -1, 1, 1.0
    "#
    .as_bytes();

    let csv_reader = csv::ReaderBuilder::new()
        .trim(csv::Trim::All) //trim whitespace around fields
        .from_reader(input_reader);

    let mut transactions_iter = transactions_from_reader(csv_reader);
    assert!(transactions_iter.any(|t| t.is_err_and(|e| matches!(e, ConversionError::CsvError(_)))));
}

#[test]
fn invalid_decimal() {
    let _ = tracing_subscriber::fmt().with_test_writer().try_init();
    //mock csv input
    let input_reader = r#"
        type, client, tx, amount
        deposit, 1, 1, 1.0
        deposit, 1, 1, -1.0
    "#
    .as_bytes();

    let csv_reader = csv::ReaderBuilder::new()
        .trim(csv::Trim::All) //trim whitespace around fields
        .from_reader(input_reader);

    let mut transactions_iter = transactions_from_reader(csv_reader);
    assert!(
        transactions_iter
            .any(|t| t.is_err_and(|e| matches!(e, ConversionError::NegativeAmount(_))))
    );
}
