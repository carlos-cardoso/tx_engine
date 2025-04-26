use criterion::{BatchSize, Bencher, Criterion, criterion_group, criterion_main};
use csv::{ReaderBuilder, WriterBuilder};
use rand::rngs::SmallRng;
use rand::{Rng, SeedableRng};
use rust_decimal::Decimal;
use rust_decimal::prelude::FromPrimitive;
use std::collections::HashSet;
use std::io::{self, Cursor, Seek, SeekFrom};
use tx_engine::csv_input::transactions_from_reader;
use tx_engine::model::{ClientId, Clients, InputCsvRecord, TransactionId};

const NUM_TRANSACTIONS_BENCH: u32 = 100_000; // We can adjust size for benchmark duration
const NUM_CLIENTS_BENCH: u16 = 10000;
const MAX_AMOUNT_BENCH: f64 = 1000.0;

// Function to generate test data records
fn generate_records(num_records: u32) -> Vec<InputCsvRecord> {
    let mut rng = SmallRng::seed_from_u64(0); // non cryptographic rng that is fast and seedable (usefull so we can have low variance when comparing performance)
    let mut records = Vec::with_capacity(num_records as usize);

    let mut deposit_transactions = HashSet::new(); // store previously generated deposits
    let mut disputed_transactions = HashSet::new(); // store previously generated disputes
    for tx_id in 1..num_records {
        let client_id = rng.random_range(1..=NUM_CLIENTS_BENCH);

        let mut tx_type: Option<String> = None;
        let mut amount: Option<Decimal> = None;

        while tx_type.is_none() {
            // retry until we get a decision
            match rng.random_range(0.0..1.0) {
                x if x < 0.5 => {
                    tx_type = Some("deposit".to_string());
                    let val = Decimal::from_f64(rng.random_range(0.01..=MAX_AMOUNT_BENCH))
                        .unwrap_or_default()
                        .round_dp(4);
                    amount = Some(val);
                    deposit_transactions.insert((tx_id, client_id));
                }
                x if x < 0.7 => {
                    tx_type = Some("withdrawal".to_string());
                    let val = Decimal::from_f64(rng.random_range(0.01..=MAX_AMOUNT_BENCH / 2.0)) // Withdraw less
                        .unwrap_or_default()
                        .round_dp(4);
                    amount = Some(val);
                }
                x if x < 0.8 => {
                    if let Some((tx_id, client_id)) = deposit_transactions.iter().next().cloned() {
                        tx_type = Some("dispute".to_string());
                        disputed_transactions.insert((tx_id, client_id));
                        deposit_transactions.remove(&(tx_id, client_id));
                        amount = None;
                    } else {
                        continue;
                    }
                }
                x if x < 0.95 => {
                    if let Some((tx_id, client_id)) = disputed_transactions.iter().next().cloned() {
                        tx_type = Some("resolve".to_string());
                        disputed_transactions.remove(&(tx_id, client_id));
                        amount = None;
                    } else {
                        continue;
                    }
                }
                _ => {
                    if let Some((tx_id, client_id)) = disputed_transactions.iter().next().cloned() {
                        tx_type = Some("chargeback".to_string());
                        disputed_transactions.remove(&(tx_id, client_id));
                        amount = None;
                    } else {
                        continue;
                    }
                }
            }
        }

        records.push(InputCsvRecord {
            transaction_type: tx_type.expect("tx_type should always be some at this point"),
            client: ClientId(client_id),
            tx: TransactionId(tx_id),
            amount,
        });
    }
    records
}

// Function to serialize records into an in-memory CSV buffer
fn create_csv_buffer(records: &[InputCsvRecord]) -> Cursor<Vec<u8>> {
    let mut buffer = Vec::new();
    {
        let mut writer = WriterBuilder::new().from_writer(Cursor::new(&mut buffer));

        for record in records {
            writer
                .serialize(record)
                .expect("Failed to serialize record for bench");
        }
        writer.flush().expect("Failed to flush writer for bench");
        // Reset the cursor position to the beginning for reading
    }
    let mut cursor = Cursor::new(buffer);
    cursor.seek(SeekFrom::Start(0)).unwrap();
    cursor
}

fn benchmark_transaction_processing(c: &mut Criterion) {
    let mut group = c.benchmark_group("CSV Processing");

    // Generate records once outside the benchmark loop if they are constant
    let records = generate_records(NUM_TRANSACTIONS_BENCH);

    group.bench_function(
        &format!("Process {} transactions in-memory", NUM_TRANSACTIONS_BENCH),
        |b: &mut Bencher| {
            // Use iter_batched to separate setup (CSV creation) from the routine (processing)
            b.iter_batched(
                || create_csv_buffer(&records),
                // part that is measured
                |mut csv_buffer| {
                    let reader = ReaderBuilder::new()
                        .trim(csv::Trim::All)
                        .from_reader(&mut csv_buffer); // Read from the buffer

                    let transactions_iter = transactions_from_reader(reader);
                    let mut clients = Clients::default();
                    // The actual work: consume the iterator and update client state
                    let transcations_result = clients.load_transactions(transactions_iter);

                    // The actual work: write output to sink
                    let output_result = clients.write(io::sink()).expect("failed to write to sink");

                    // Use black_box to prevent the compiler optimizing away the result
                    criterion::black_box(transcations_result);
                    criterion::black_box(output_result);
                    criterion::black_box(clients); // Also prevent clients from being optimized away
                },
                BatchSize::SmallInput,
            );
        },
    );

    group.finish();
}

criterion_group!(benches, benchmark_transaction_processing);
criterion_main!(benches);
