use std::{collections::HashMap, env, path::Path};

use tx_engine::{
    csv_input::read_transactions_from_csv,
    model::{Account, ClientId},
};

fn main() {
    let mut args = env::args();

    let file_path = args.nth(1).expect("No command line argument was provided");
    let file_path = Path::new(&file_path);
    let transactions_iter = read_transactions_from_csv(file_path).expect("failed to load the csv");

    let mut clients: HashMap<ClientId, Account> = HashMap::new();

    for t in transactions_iter {
        println!("{:?}", t);
        let transaction = t.expect("invalid transaction");

        let client_id = transaction.client_id();
        clients
            .entry(client_id)
            .and_modify(|account| account.apply(&transaction))
            .or_insert_with(|| {
                let mut account = Account::new();
                account.apply(&transaction);
                account
            });
    }
    println!("{:?}", clients);
}
