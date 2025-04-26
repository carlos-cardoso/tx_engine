use std::{collections::HashMap, fmt::Display, io};

use rust_decimal::{Decimal, dec};
use serde::{Deserialize, Serialize};
use tracing::{Level, error, instrument, span, trace, warn};

use crate::csv_input::ConversionError;

/// Clients contains the mapping between the ClientId's and the Client Accounts
#[derive(Debug, Default)]
pub struct Clients {
    pub accounts: HashMap<ClientId, Account>, // Client accounts
    pub disputable_transactions: HashMap<TransactionId, TransactionStatus>, // Transactions that can be disputed or resolved or chargedback (shared since TransactionIds are globally unique)
}

impl Clients {
    /// Mutate the client Accounts with an iterator over Transactions
    #[instrument(skip(transactions))]
    pub fn load_transactions<T: Iterator<Item = Result<Transaction, ConversionError>>>(
        &mut self,
        transactions: T,
    ) {
        for transaction in transactions {
            match transaction {
                Err(err) => error!(error=%err, "Skipping invalid transaction in file"),
                Ok(transaction) => {
                    let client_id = transaction.client_id();
                    let span = span!(Level::TRACE, "applying transaction");
                    let _enter = span.enter();
                    self.accounts
                        .entry(client_id)
                        .and_modify(|account| {
                            account.apply(&transaction, &mut self.disputable_transactions)
                        })
                        .or_insert_with(|| {
                            let mut account = Account::default();
                            account.apply(&transaction, &mut self.disputable_transactions);
                            account
                        });
                }
            }
        }
    }

    /// Write the output csv to a Writer (e.g. to stdout)
    pub fn write<W: io::Write>(&self, wtr: W) -> io::Result<()> {
        let mut csv_writer = csv::WriterBuilder::new().from_writer(wtr);

        for (client, account) in self.accounts.iter() {
            csv_writer.serialize(CsvOutputAccount::from((client, account)))?;
        }

        csv_writer.flush()?;
        Ok(())
    }
}

// Possible states of a disputable transaction
#[derive(Debug)]
pub enum TransactionStatus {
    NotDisputedAmount(Decimal),
    DisputedAmount(Decimal),
}

impl Account {
    #[instrument]
    pub fn apply(
        &mut self,
        transaction: &Transaction,
        disputable_transactions: &mut HashMap<TransactionId, TransactionStatus>,
    ) {
        match transaction {
            Transaction::Deposit {
                client: _,
                tx,
                amount,
            } => {
                self.available += amount;
                disputable_transactions.insert(
                    tx.to_owned(),
                    TransactionStatus::NotDisputedAmount(amount.to_owned()),
                );
                trace!("Applied deposit");
            }
            Transaction::Withdrawal {
                client: _,
                tx: _,
                amount,
            } => {
                if self.available >= *amount {
                    self.available -= amount;
                    trace!(%amount, "Applied whitdrawal");
                } else {
                    warn!(%amount, %self.available, "not enough funds available for whithdrawal")
                }
            }
            Transaction::Dispute { client: _, tx } => {
                if let Some(disputable_transaction) = disputable_transactions.get_mut(tx) {
                    if let TransactionStatus::NotDisputedAmount(amount) = disputable_transaction {
                        self.held += *amount;
                        self.available -= *amount;
                        *disputable_transaction = TransactionStatus::DisputedAmount(*amount);
                        trace!(%tx, "Disputed transaction");
                    } else {
                        warn!(%tx, ?disputable_transaction, "Transaction cannot be disputed");
                    }
                } else {
                    warn!(%tx, "transaction does not exist in disputable transactions");
                }
            }
            Transaction::Resolve { client: _, tx } => {
                if let Some(disputable_transaction) = disputable_transactions.get_mut(tx) {
                    if let TransactionStatus::DisputedAmount(amount) = disputable_transaction {
                        self.held -= *amount;
                        self.available += *amount;
                        *disputable_transaction = TransactionStatus::NotDisputedAmount(*amount);
                        trace!(%tx, "Resolved transaction");
                    } else {
                        warn!(%tx, ?disputable_transaction, "Transaction cannot be resolved");
                    }
                } else {
                    warn!(%tx, "transaction does not exist in disputable transactions");
                }
            }
            Transaction::Chargeback { client: _, tx } => {
                if let Some(disputable_transaction) = disputable_transactions.get_mut(tx) {
                    if let TransactionStatus::DisputedAmount(amount) = disputable_transaction {
                        self.held -= *amount;
                        disputable_transactions.remove(tx); // if a transaction was charged back then it cannot be disputed again
                        trace!(%tx, "Transaction was chargedback");
                    } else {
                        warn!(%tx, ?disputable_transaction, "Transaction cannot be charged back");
                    }
                    self.locked = true;
                    trace!(%tx, "Account locked");
                } else {
                    warn!(%tx, "transaction does not exist in disputable transactions");
                }
            }
        }
    }
}

#[derive(Debug, Serialize, PartialEq, Eq, Clone)]
pub struct Account {
    available: Decimal, // The total funds that are available for trading, staking, withdrawal, etc. This should be equal to the total - held amount
    held: Decimal, // The total funds that are held for dispute. This should be equal to total - available amounts
    locked: bool,  // Whether the account is locked. An account is locked if a charge back occurs
}

#[derive(Debug, Serialize, PartialEq, Eq, Clone)]
pub struct CsvOutputAccount {
    client: ClientId,
    available: Decimal, // The total funds that are available for trading, staking, withdrawal, etc. This should be equal to the total - held amount
    held: Decimal, // The total funds that are held for dispute. This should be equal to total - available amounts
    total: Decimal, // The total funds that are available or held. This should be equal to available + held
    locked: bool,   // Whether the account is locked. An account is locked if a charge back occurs
}

impl From<(&ClientId, &Account)> for CsvOutputAccount {
    fn from(value: (&ClientId, &Account)) -> Self {
        Self {
            client: value.0.clone(),
            available: value.1.available(),
            held: value.1.held(),
            total: value.1.total(),
            locked: value.1.locked(),
        }
    }
}

impl Default for Account {
    fn default() -> Self {
        Account {
            held: dec!(0),
            available: dec!(0),
            locked: false,
        }
    }
}

impl Account {
    pub fn new(available: Decimal, held: Decimal, locked: bool) -> Account {
        Account {
            available,
            held,
            locked,
        }
    }

    /// Banker's rounding, also known as round-to-even, is a rounding method where numbers equidistant
    /// from two integers are rounded to the nearest even integer.
    /// This method is particularly useful in financial and statistical calculations to minimize bias and cumulative errors
    pub fn available(&self) -> Decimal {
        self.available.round_dp(4) // bankers rounding 0.00025 -> 0.0002  and 0.00015 -> 0.0002
    }

    pub fn held(&self) -> Decimal {
        self.held.round_dp(4) // bankers rounding 0.00025 -> 0.0002  and 0.00015 -> 0.0002
    }

    pub fn total(&self) -> Decimal {
        self.available.round_dp(4) + self.held.round_dp(4) // bankers rounding 0.00025 -> 0.0002  and 0.00015 -> 0.0002
    }

    pub fn locked(&self) -> bool {
        self.locked
    }
}

#[derive(Debug, Deserialize, PartialEq, Eq, Hash, Clone, Serialize)]
pub struct ClientId(pub u16);

impl Display for ClientId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

#[derive(Debug, Deserialize, PartialEq, Eq, Hash, Clone, Serialize)]
pub struct TransactionId(pub u32);

impl Display for TransactionId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

#[derive(Debug)]
pub enum Transaction {
    /// A deposit is a credit to the client's asset account, meaning it should increase the available
    /// and total funds of the client account
    Deposit {
        client: ClientId,
        tx: TransactionId,
        amount: Decimal,
    },
    /// A withdraw is a debit to the client's asset account, meaning it should decrease the available
    /// and total funds of the client account
    /// If a client does not have sufficient available funds the withdrawal should fail and the total
    /// amount of funds should not change
    Withdrawal {
        client: ClientId,
        tx: TransactionId,
        amount: Decimal,
    },
    /// A dispute represents a client's claim that a transaction was erroneous and should be
    /// reversed. The transaction shouldn't be reversed yet but the associated funds should be
    /// held. This means that the clients' available funds should decrease by the amount disputed,
    /// their held funds should increase by the amount disputed, while their total funds should
    /// remain the same.
    /// a dispute does not state the amount disputed. Instead a dispute references
    /// the transaction that is disputed by ID. If the tx specified by the dispute doesn't exist you
    /// can ignore it and assume this is an error on our partners side.
    Dispute { client: ClientId, tx: TransactionId },
    /// A resolve represents a resolution to a dispute, releasing the associated held funds. Funds
    /// that were previously disputed are no longer disputed. This means that the clients held funds
    /// should decrease by the amount no longer disputed, their available funds should increase by
    /// the amount no longer disputed, and their total funds should remain the same.
    /// resolves do not specify an amount. Instead they refer to a transaction that
    /// was under dispute by ID. If the tx specified doesn't exist, or the tx isn't under dispute, you
    /// can ignore the resolve and assume this is an error on our partner's side.
    Resolve { client: ClientId, tx: TransactionId },
    /// A chargeback is the final state of a dispute and represents the client reversing a transaction.
    /// Funds that were held have now been withdrawn. This means that the clients held funds and
    /// total funds should decrease by the amount previously disputed. If a chargeback occurs the
    /// client's account should be immediately frozen.
    /// Like a dispute and a resolve a chargeback refers to the transaction by ID (tx) and does not
    /// specify an amount. Like a resolve, if the tx specified doesn't exist, or the tx isn't under
    /// dispute, you can ignore chargeback and assume this is an error on our partner's side.
    Chargeback { client: ClientId, tx: TransactionId },
}

impl Transaction {
    pub fn client_id(&self) -> ClientId {
        match self {
            Transaction::Deposit {
                client,
                tx: _,
                amount: _,
            } => client,
            Transaction::Withdrawal {
                client,
                tx: _,
                amount: _,
            } => client,
            Transaction::Dispute { client, tx: _ } => client,
            Transaction::Resolve { client, tx: _ } => client,
            Transaction::Chargeback { client, tx: _ } => client,
        }
        .clone()
    }
}

/// Type used to deserialize input csv lines
#[derive(Debug, Deserialize, Serialize)]
pub struct InputCsvRecord {
    #[serde(rename = "type")]
    pub transaction_type: String,
    pub client: ClientId,
    pub tx: TransactionId,
    pub amount: Option<Decimal>,
}

/// Converts from an InputCsvRecord to a Transaction
impl TryFrom<InputCsvRecord> for Transaction {
    fn try_from(csv_record: InputCsvRecord) -> Result<Self, ConversionError> {
        let InputCsvRecord {
            transaction_type,
            client,
            tx,
            amount,
        } = csv_record;
        Ok(match transaction_type.as_str() {
            "deposit" => {
                let amount =
                    amount.ok_or(ConversionError::MissingAmount(transaction_type.to_string()))?;

                // handle negative amount
                if amount.is_sign_negative() {
                    return Err(ConversionError::NegativeAmount(format!(
                        "deposited amount: {amount} must be positive"
                    )));
                }
                Transaction::Deposit { client, tx, amount }
            }
            "withdrawal" => {
                let amount =
                    amount.ok_or(ConversionError::MissingAmount(transaction_type.to_string()))?;

                // handle negative amount
                if amount.is_sign_negative() {
                    return Err(ConversionError::NegativeAmount(format!(
                        "withdrawal amount: {amount} must be positive"
                    )));
                }
                Transaction::Withdrawal { client, tx, amount }
            }
            "dispute" => Transaction::Dispute { client, tx },
            "resolve" => Transaction::Resolve { client, tx },
            "chargeback" => Transaction::Chargeback { client, tx },
            _ => Err(ConversionError::InvalidTransactionType(
                transaction_type.to_string(),
            ))?,
        })
    }

    type Error = ConversionError;
}
