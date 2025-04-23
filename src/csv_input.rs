use csv::Reader;
use model::{InputCsvRecord, Transaction};
use std::{fs::File, path::Path};
use thiserror::Error;

use crate::model;

#[derive(Error, Debug)]
pub enum ConversionError {
    #[error("Missing mount for transaction type: {0}")]
    MissingAmount(String),

    #[error("Invalid transaction type: {0}")]
    InvalidTransactionType(String),

    #[error("CSV parsing error")]
    CsvError(#[from] csv::Error),

    #[error("Failed to parse decimal amount")]
    ParseDecimal(#[from] rust_decimal::Error),

    #[error("Decimal amount must be positive")]
    NegativeAmount(String),

    #[error("An unexpected error occurred: {0}")]
    Unexpected(String), // Catch-all if needed
}

// Loads the csv in path as a Iterator over transactions
pub fn read_transactions_from_csv(
    csv_path: &Path,
) -> Result<impl Iterator<Item = Result<Transaction, ConversionError>>, ConversionError> {
    let csv_reader = csv::ReaderBuilder::new()
        .trim(csv::Trim::All) //trim whitespace around fields
        .from_path(csv_path)
        .map_err(ConversionError::from)?;

    Ok(transactions_from_reader(csv_reader))
}

// Transforms a reader over a file into a iterator over transactions
pub fn transactions_from_reader<T: std::io::Read>(
    csv_reader: Reader<T>,
) -> impl Iterator<Item = Result<Transaction, ConversionError>> {
    csv_reader
        .into_deserialize()
        .map(|record: Result<InputCsvRecord, csv::Error>| {
            let csv_record: InputCsvRecord = record.map_err(ConversionError::from)?;
            Transaction::try_from(csv_record)
        })
}
