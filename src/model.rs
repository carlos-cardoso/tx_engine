use std::{
    collections::HashMap,
    fmt::Display,
    ops::Not,
    sync::mpsc::{SendError, Sender},
};

use rust_decimal::{Decimal, dec};
use serde::{Deserialize, Serialize};
use tracing::{Level, error, instrument, span, trace, warn};

use crate::csv_input::ConversionError;

/// Clients contains the mapping between the ClientId's and the Client Accounts
#[derive(Debug)]
pub struct Clients {
    pub accounts: HashMap<ClientId, Account>, // Client accounts
    pub disputable_transactions: HashMap<TransactionId, DisputableTransactionStatus>, // Transactions that can be disputed or resolved or chargedback (shared since TransactionIds are globally unique)
    pub output_sender: Sender<(ClientId, Account)>, // sender to early print accounts that are in a final state (locked)
}

impl Clients {
    pub fn new(tx: Sender<(ClientId, Account)>) -> Clients {
        Clients {
            accounts: HashMap::new(),
            disputable_transactions: HashMap::new(),
            output_sender: tx,
        }
    }

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
                            if account.locked().not() {
                                //if not locked
                                account.apply(&transaction, &mut self.disputable_transactions);
                                if account.locked() { // became locked, we can send this account to the output imediately
                                    self.output_sender
                                        .send((client_id, account.clone()))
                                        .expect("failed to send");
                                }
                            }
                            else{
                                warn!(%client_id, ?transaction, "Tried to apply transction to a locked account");
                            }
                        })
                        .or_insert_with(|| {
                            let mut account = Account::default();
                            account.apply(&transaction, &mut self.disputable_transactions);
                            if account.locked() { // became locked, we can send this account to the output imediately
                                self.output_sender
                                    .send((client_id, account.clone()))
                                    .expect("failed to send");
                            }
                            account
                        });
                }
            }
        }
    }

    /// Send accounts to the output channel
    pub fn send_to_output(
        self,
        output_mode: OutputMode, // Send All the accounts or skip the locked ones
    ) -> Result<(), SendError<(ClientId, Account)>> {
        for (client, account) in self
            .accounts
            .into_iter()
            .filter(|(_, account)| matches!(output_mode, OutputMode::All) || account.locked().not())
        {
            self.output_sender.send((client, account))?;
        }
        Ok(())
    }
}

// Output all accounts or skip the locked ones
pub enum OutputMode {
    SkipLocked,
    All,
}

// Possible states of a disputable transaction (deposit)
// Criterion shows that there is a performance gain (6%) in not having a ChargedBack variant and simply
// removing transactions that were charged back
// the Decimal is the ammount involved in the deposit
#[derive(Debug)]
pub enum DisputableTransactionStatus {
    NotDisputedAmount(Decimal),
    DisputedAmount(Decimal),
}

impl Account {
    fn apply_deposit(
        &mut self,
        tx: TransactionId,
        amount: Decimal,
        disputable_transactions: &mut HashMap<TransactionId, DisputableTransactionStatus>,
    ) {
        self.available += amount;
        disputable_transactions.insert(tx, DisputableTransactionStatus::NotDisputedAmount(amount));
        trace!("Applied deposit");
    }

    fn apply_whithdrawal(&mut self, amount: Decimal) {
        if self.available >= amount {
            self.available -= amount;
            trace!(%amount, "Applied whitdrawal");
        } else {
            warn!(%amount, %self.available, "not enough funds available for whithdrawal")
        }
    }
    fn apply_dispute(
        &mut self,
        tx: &TransactionId,
        disputable_transactions: &mut HashMap<TransactionId, DisputableTransactionStatus>,
    ) {
        match disputable_transactions.get_mut(tx) {
            // Transaction exists
            Some(status) => match status {
                // It's currently not disputed, so we can dispute it
                DisputableTransactionStatus::NotDisputedAmount(amount) => {
                    self.held += *amount;
                    self.available -= *amount;
                    *status = DisputableTransactionStatus::DisputedAmount(*amount);
                    trace!(%tx, "Disputed transaction");
                }
                // It's already disputed or in another invalid state
                DisputableTransactionStatus::DisputedAmount(_) => {
                    warn!(%tx, ?status, "Transaction is already disputed or cannot be disputed");
                }
            },
            // Transaction does not exist in the map
            None => {
                warn!(%tx, "Dispute references a non-existent or non-disputable transaction");
            }
        }
    }

    fn apply_resolve(
        &mut self,
        tx: &TransactionId,
        disputable_transactions: &mut HashMap<TransactionId, DisputableTransactionStatus>,
    ) {
        match disputable_transactions.get_mut(tx) {
            // Transaction exists
            Some(status) => match status {
                DisputableTransactionStatus::DisputedAmount(amount) => {
                    self.held -= *amount;
                    self.available += *amount;
                    *status = DisputableTransactionStatus::NotDisputedAmount(*amount);
                    trace!(%tx, "Resolved transaction");
                }
                DisputableTransactionStatus::NotDisputedAmount(_) => {
                    warn!(%tx, ?status, "Transaction is not disputed: it cannot be resolved");
                }
            },
            None => {
                warn!(%tx, "transaction does not exist in disputable transactions");
            }
        }
    }

    fn apply_chargeback(
        &mut self,
        tx: &TransactionId,
        disputable_transactions: &mut HashMap<TransactionId, DisputableTransactionStatus>,
    ) {
        match disputable_transactions.get_mut(tx) {
            Some(status) => match status {
                DisputableTransactionStatus::DisputedAmount(amount) => {
                    self.held -= *amount;
                    disputable_transactions.remove(tx); // if a transaction was charged back then it cannot be disputed again
                    trace!(%tx, "Transaction was chargedback");

                    self.locked = true; // according to the specification we can ignore chargeback if the tx does not exist or is not in dispute, by extension we also do not lock the account
                    trace!(%tx, "Account locked");
                }
                DisputableTransactionStatus::NotDisputedAmount(_) => {
                    warn!(%tx, ?status, "Transaction is not disputed: cannot be charged back");
                }
            },
            None => {
                warn!(%tx, "transaction does not exist in disputable transactions");
            }
        }
    }

    #[instrument]
    /// Mutate this account with a transaction
    pub fn apply(
        &mut self,
        transaction: &Transaction,
        disputable_transactions: &mut HashMap<TransactionId, DisputableTransactionStatus>, // map that keeps the transactions that are disputable or in dispute
    ) {
        if self.locked.not() {
            // if account is not locked
            match transaction {
                Transaction::Deposit {
                    client: _,
                    tx,
                    amount,
                } => {
                    self.apply_deposit(*tx, *amount, disputable_transactions);
                }
                Transaction::Withdrawal {
                    client: _,
                    tx: _,
                    amount,
                } => {
                    self.apply_whithdrawal(*amount);
                }
                Transaction::Dispute { client: _, tx } => {
                    self.apply_dispute(tx, disputable_transactions);
                }
                Transaction::Resolve { client: _, tx } => {
                    self.apply_resolve(tx, disputable_transactions);
                }
                Transaction::Chargeback { client: _, tx } => {
                    self.apply_chargeback(tx, disputable_transactions);
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
            client: *value.0,
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

#[derive(Debug, Deserialize, PartialEq, Eq, Hash, Clone, Serialize, Copy)]
pub struct ClientId(pub u16);

impl Display for ClientId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

#[derive(Debug, Deserialize, PartialEq, Eq, Hash, Clone, Serialize, Copy)]
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
        .to_owned()
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
