//! End-to-end fixtures of the Rust port: the bank-account domain of the
//! Scala `e2e` module (`modules/e2e/src/main/scala/accounts/*.scala`) with
//! the JSON shape Circe gives it, so that Scala and Rust services can share
//! a journal. Used by `tests/e2e.rs` and by the cross-language test.

#![forbid(unsafe_code)]

use edomata_core::{App, Decision, DomainModel, NonEmpty};
pub use rust_decimal::Decimal as Amount;
use serde::{Deserialize, Serialize};

/// Bank-account events. Parameterless cases serialize as `{"Opened":{}}`,
/// like Circe's generic derivation of Scala 3 enums.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Event {
    /// The account was opened.
    Opened {},
    /// Money came in.
    Deposited {
        /// Amount deposited.
        #[serde(with = "rust_decimal::serde::float")]
        amount: Amount,
    },
    /// Money went out.
    Withdrawn {
        /// Amount withdrawn.
        #[serde(with = "rust_decimal::serde::float")]
        amount: Amount,
    },
    /// The account was closed.
    Closed {},
}

/// Why a command is rejected.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Rejection {
    /// Opening an existing account.
    ExistingAccount,
    /// Operating on an account that does not exist.
    NoSuchAccount,
    /// Withdrawing more than the balance (or a non-positive amount).
    InsufficientBalance,
    /// Closing an account with a non-zero balance.
    NotSettled,
    /// Operating on a closed account.
    AlreadyClosed,
    /// A non-positive deposit.
    BadRequest,
}

/// Account state.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Account {
    /// Not opened yet.
    New {},
    /// Open with a balance.
    Open {
        /// Current balance.
        #[serde(with = "rust_decimal::serde::float")]
        balance: Amount,
    },
    /// Closed.
    Close {},
}

impl Account {
    /// An open account with the given balance.
    pub fn open_with(balance: impl Into<Amount>) -> Self {
        Account::Open {
            balance: balance.into(),
        }
    }

    fn must_be_open(&self) -> Result<Amount, NonEmpty<Rejection>> {
        match self {
            Account::Open { balance } => Ok(*balance),
            Account::New {} => Err(NonEmpty::new(Rejection::NoSuchAccount)),
            Account::Close {} => Err(NonEmpty::new(Rejection::AlreadyClosed)),
        }
    }

    /// Opens the account; yields the (zero) balance.
    pub fn open(&self) -> Decision<Rejection, Event, Amount> {
        let decision = match self {
            Account::New {} => Decision::accept(Event::Opened {}),
            _ => Decision::reject(Rejection::ExistingAccount),
        };
        AccountModel
            .perform(self.clone(), decision)
            .validate_with(|s| s.must_be_open())
    }

    /// Closes a settled account.
    pub fn close(&self) -> Decision<Rejection, Event, Account> {
        let decision = self
            .must_be_open()
            .map_or_else(Decision::Rejected, |balance| {
                if balance.is_zero() {
                    Decision::accept(Event::Closed {})
                } else {
                    Decision::reject(Rejection::NotSettled)
                }
            });
        AccountModel.perform(self.clone(), decision)
    }

    /// Withdraws money; yields the new balance.
    pub fn withdraw(&self, amount: Amount) -> Decision<Rejection, Event, Amount> {
        let decision = self
            .must_be_open()
            .map_or_else(Decision::Rejected, |balance| {
                if balance >= amount && amount.is_sign_positive() && !amount.is_zero() {
                    Decision::accept(Event::Withdrawn { amount })
                } else {
                    Decision::reject(Rejection::InsufficientBalance)
                }
            });
        AccountModel
            .perform(self.clone(), decision)
            .validate_with(|s| s.must_be_open())
    }

    /// Deposits money; yields the new balance.
    pub fn deposit(&self, amount: Amount) -> Decision<Rejection, Event, Amount> {
        let decision = self.must_be_open().map_or_else(Decision::Rejected, |_| {
            if amount.is_sign_positive() && !amount.is_zero() {
                Decision::accept(Event::Deposited { amount })
            } else {
                Decision::reject(Rejection::BadRequest)
            }
        });
        AccountModel
            .perform(self.clone(), decision)
            .validate_with(|s| s.must_be_open())
    }
}

/// The account domain model.
#[derive(Clone, Copy, Debug, Default)]
pub struct AccountModel;

impl DomainModel for AccountModel {
    type State = Account;
    type Event = Event;
    type Rejection = Rejection;

    fn initial(&self) -> Account {
        Account::New {}
    }

    fn transition(&self, event: &Event, state: Account) -> Result<Account, NonEmpty<Rejection>> {
        match event {
            Event::Opened {} => Ok(Account::open_with(Amount::ZERO)),
            Event::Withdrawn { amount } => {
                state.must_be_open().map(|b| Account::open_with(b - amount))
            }
            Event::Deposited { amount } => {
                state.must_be_open().map(|b| Account::open_with(b + amount))
            }
            Event::Closed {} => Ok(Account::Close {}),
        }
    }
}

/// Account commands.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Command {
    /// Open the account.
    Open,
    /// Deposit an amount.
    Deposit(Amount),
    /// Withdraw an amount.
    Withdraw(Amount),
    /// Close the account.
    Close,
}

/// Integration events published to the outbox, with Circe's field names.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Notification {
    /// An account was opened.
    AccountOpened {
        /// The account.
        #[serde(rename = "accountId")]
        account_id: String,
    },
    /// The balance changed.
    BalanceUpdated {
        /// The account.
        #[serde(rename = "accountId")]
        account_id: String,
        /// The new balance.
        #[serde(with = "rust_decimal::serde::float")]
        balance: Amount,
    },
    /// An account was closed.
    AccountClosed {
        /// The account.
        #[serde(rename = "accountId")]
        account_id: String,
    },
}

/// The account service program type.
pub type AccountApp = App<Command, Account, Event, Rejection, Notification, ()>;

/// Port of `AccountService`: routes commands to the domain and publishes
/// notifications.
pub fn account_service() -> AccountApp {
    let dsl = AccountModel.dsl::<Command, Notification>();
    dsl.router(move |cmd| match cmd {
        Command::Open => dsl
            .state()
            .and_then(move |s| dsl.decide(s.open()))
            .and_then(move |_| dsl.aggregate_id())
            .and_then(move |account_id| dsl.publish([Notification::AccountOpened { account_id }])),
        Command::Deposit(amount) => dsl
            .state()
            .and_then(move |s| dsl.decide(s.deposit(amount)))
            .and_then(move |balance| {
                dsl.aggregate_id().and_then(move |account_id| {
                    dsl.publish([Notification::BalanceUpdated {
                        account_id,
                        balance,
                    }])
                })
            }),
        Command::Withdraw(amount) => dsl
            .state()
            .and_then(move |s| dsl.decide(s.withdraw(amount)))
            .and_then(move |balance| {
                dsl.aggregate_id().and_then(move |account_id| {
                    dsl.publish([Notification::BalanceUpdated {
                        account_id,
                        balance,
                    }])
                })
            }),
        Command::Close => dsl.state().and_then(move |s| dsl.decide(s.close())).void(),
    })
}

/// Connects to `DATABASE_URL` or the docker-compose default.
pub async fn pool() -> edomata_sqlx::PgPool {
    let url = std::env::var("DATABASE_URL")
        .unwrap_or_else(|_| "postgres://postgres:postgres@localhost:5432/postgres".to_string());
    sqlx::postgres::PgPoolOptions::new()
        .max_connections(8)
        .connect(&url)
        .await
        .expect("PostgreSQL from docker-compose must be running (see rust/README.md)")
}

/// A random aggregate address.
pub fn random_string() -> String {
    uuid::Uuid::new_v4().to_string()
}
