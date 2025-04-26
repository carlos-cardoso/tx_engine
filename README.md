# Readme

# tx_engine: Toy Payments Engine

## Overview

This project implements a simple toy payments engine written in Rust. It processes a stream of transaction operations from a CSV file, manages client account balances (available, held, total), handles disputes and resolutions, and outputs the final state of all client accounts to standard output as a CSV.

## Project Structure:
```text
tree
.
├── benches
│   └── transaction_processing.rs
├── Cargo.lock
├── Cargo.toml
├── data
│   └── input_example.csv
├── README.md
├── src
│   ├── csv_input.rs
│   ├── lib.rs
│   ├── main.rs
│   └── model.rs
└── tests
    ├── test_csv.rs
    └── test_process_transactions.rs

5 directories, 11 files
```

## Input Example:
```csv
type, client, tx, amount
deposit, 1, 1, 1.0
deposit, 2, 2, 2.0
deposit, 1, 3, 2.0
withdrawal, 1, 4, 1.5
withdrawal, 2, 5, 3.0
```

## Output Example: (Stdout)

```csv
client,available,held,total,locked
2,2,0,2,false
1,1.5,0,1.5,false
```

(Note: The order of clients in the output is not guaranteed.)

## Usage:

1. Run the project against sample:

```bash
 cargo run --release -- data/input_example.csv > out.csv
```

2. Run the tests:

```bash
 cargo tests
```

3. Run the benchmarks (criterion):

```bash
 cargo bench
```

4. See the Logs: (error, info, warn, trace) 

```bash
 RUST_LOG=info cargo run --release -- data/input_example.csv > out.csv
```


## Design Notes & Assumptions:

  - Client IDs: Clients are represented by u16 integers. New client records are created automatically if a transaction references a non-existent client.
  - Transaction IDs: Transaction IDs (u32) are assumed to be globally unique for transaction types that introduce funds.
  - Amount Precision: Uses rust_decimal with a scale of 4 for financial calculations.

#### Error Handling:

  - Invalid transaction types or formats in the input CSV are logged as warnings and skipped.
  - Attempts to withdraw more funds than available are logged and ignored.
  - Disputes/Resolves/Chargebacks referencing non-existent transactions or transactions not in the correct state (e.g., resolving a non-disputed transaction) are logged and ignored.
  - Once a account is locked by a chargeback it cannot process more transactions. If a transaction is applied to a locked account a log is produced.

  - Concurrency: The current implementation processes transactions sequentially from the input CSV. It uses an iterator to avoid loading the entire file in memory.
                 The writer runs in a dedicated thread, and starts printing the accounts that are locked.
                 After reaching the end of the input file all accounts that were not printed already are then finally printed.

  - Dependencies: Uses csv, serde, rust_decimal, thiserror and tracing crates. For benchmarking it uses criterion and rand. 

## Concurrency Model
    - **Sequential Processing**: The core transaction processing logic reads and handles transactions one by one from the input stream. Given the sequential nature of the input CSV and the dependency of transaction outcomes on prior states for a given client, parallelizing the processing of transactions for the same client is complex and not implemented. Parallelizing processing across different clients could be possible but adds complexity. There would also be little gain since we need to wait for the end of the file to know that the client account will not be further modified.
    - **Dedicated Writer Thread**: A separate thread handles writing the output CSV records to stdout. This allows the main processing thread to continue handling transactions while output is being written concurrently. Locked accounts can be written out immediately by the writer thread once the chargeback is processed, potentially reducing overall execution time and memory pressure for scenarios with many locked accounts.

## Benchmarking: Dedicated Writer Thread

The impact of the dedicated writer thread was measured using hyperfine on a Linux laptop (Ryzen 9) with a large generated CSV file (100 million transactions, ~2.9GB) with 5% of probability of a transaction being a chargeback.

```bash
wc -l testfile.csv
100000000 testfile.csv
ls -lh testfile.csv
-rw-r--r-- 1 carlos carlos 2,9G 26. Apr 21:39 testfile.csv
```

#### Baseline (Process all then Write):

```bash
Benchmark 1: cargo run --release -- /home/carlos/testfile.csv
  Time (mean ± σ):     52.683 s ±  1.262 s    [User: 51.784 s, System: 0.807 s]
  Range (min … max):   51.150 s … 55.401 s    10 runs
```

#### With Dedicated writer Thread (~21% faster): 

```bash
Benchmark 1: cargo run --release -- /home/carlos/testfile.csv
  Time (mean ± σ):     41.619 s ±  1.632 s    [User: 40.964 s, System: 0.816 s]
  Range (min … max):   39.627 s … 44.459 s    10 runs
```

## Potential Future Optimizations / Alternative Designs

   - **Async Processing**: If the input source were different (e.g., network streams, message queues), an async approach (e.g., using Tokio) would be suitable for I/O-bound operations.
   - **Database Backend**: For persistence, larger scale, or more complex queries, integrating a database (SQL or NoSQL Key/Value store) would be necessary.
   - **Event Sourcing / CQRS**: For high-throughput systems or systems requiring detailed audit trails, Event Sourcing could be employed. Commands (transactions) generate events stored immutably. Account states (read models) would be derived from these events, potentially using Command Query Responsibility Segregation (CQRS) to optimize read and write paths separately.
