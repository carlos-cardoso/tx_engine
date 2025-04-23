use std::collections::HashMap;

use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};

use crate::csv_input::ConversionError;

#[derive(Debug)]
pub struct Account {
    disputable_transactions: HashMap<TransactionId, Decimal>,
    available: Decimal, // The total funds that are available for trading, staking, withdrawal, etc. This should be equal to the total - held amount
    held: Decimal, // The total funds that are held for dispute. This should be equal to total - available amounts
    total: Decimal, // The total funds that are available or held. This should be equal to available + held
    locked: bool,   // Whether the account is locked. An account is locked if a charge back occurs
}

impl Account {
    pub fn new() -> Self {
        Account {
            disputable_transactions: HashMap::new(),
            total: Decimal::new(0, 4),
            held: Decimal::new(0, 4),
            available: Decimal::new(0, 4),
            locked: false,
        }
    }

    fn update_total(&mut self) {
        self.total = self.held + self.available;
    }

    // pub fn apply<T: Iterator<Item = Transaction>>(&mut self, transactions: T) {
    pub fn apply(&mut self, transaction: &Transaction) {
        match transaction {
            Transaction::Deposit {
                client: _,
                tx,
                amount,
            } => {
                self.available = self.available + amount;
                self.update_total();
            }
            Transaction::Withdrawal {
                client: _,
                tx,
                amount,
            } => {
                if self.available >= *amount {
                    self.available = self.available - amount;
                }
                self.update_total();
            }
            Transaction::Dispute { client: _, tx } => {
                todo!()
            }
            Transaction::Resolve { client: _, tx } => {
                todo!()
            }
            Transaction::Chargeback { client: _, tx } => {
                todo!()
            }
        }
    }
}

#[derive(Debug, Deserialize, PartialEq, Eq, Hash, Clone, Serialize)]
pub struct ClientId(u16);

#[derive(Debug, Deserialize)]
pub struct TransactionId(u32);

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
#[derive(Debug, serde::Deserialize)]
pub struct InputCsvRecord {
    #[serde(rename = "type")]
    transaction_type: String,
    client: ClientId,
    tx: TransactionId,
    amount: Option<Decimal>,
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
