//! Samples of the "Event sourcing" chapter.

#![allow(clippy::let_and_return)]

// ANCHOR: imports
use edomata_core::syntax::*; // convenient extension methods
use edomata_core::*;
use serde::{Deserialize, Serialize};
// ANCHOR_END: imports

/// The `Decision` examples of the chapter.
pub fn decisions() {
    // ANCHOR: decisions
    let d1: Decision<String, String, i32> = Decision::pure(1);
    let d2: Decision<String, String, ()> = Decision::accept("Missile Launched!".to_string());
    let d3: Decision<String, String, ()> =
        Decision::reject("No remained missiles to launch!".to_string());
    // ANCHOR_END: decisions

    // ANCHOR: decision_syntax
    let s1: Decision<String, String, i32> = 1.into_decision();
    let s2: Decision<String, String, ()> = "Missile Launched!".to_string().accept();
    let s3: Decision<String, String, ()> = "No remained missiles to launch!".to_string().reject();
    // ANCHOR_END: decision_syntax

    // ANCHOR: decision_compose
    let d4 = d1.clone().map(|i| i * 2);
    let d5 = d2.clone().then(d1.clone()); // `>>` in Scala
    let d6 = d5.clone().then(d3.clone());
    // ANCHOR_END: decision_compose

    // ANCHOR: decision_chain
    // The for-comprehension of the Scala tutorial, written with `and_then`.
    let d7: Decision<String, String, i32> = Decision::pure(1).and_then(|i| {
        Decision::accept("A".to_string()) // accepting one event
            .then(Decision::accept(nonempty![
                "B".to_string(),
                "C".to_string()
            ])) // several events
            .then(Decision::accept_return(
                i * 2,
                nonempty!["D".to_string(), "E".to_string()],
            )) // several events and a value
            .map(move |j| i + j)
    });
    // ANCHOR_END: decision_chain

    assert_eq!(d4.result(), Some(&2));
    assert!(d5.is_accepted());
    assert!(d6.is_rejected());
    assert_eq!(d7.result(), Some(&3));
    assert_eq!(d7.events().map(|e| e.as_slice().len()), Some(5));
    assert_eq!(s1, d1);
    assert_eq!(s2, d2);
    assert_eq!(s3, d3);
}

// ANCHOR: events
/// What happened to an account. Events are named in the past tense: by the
/// time we record them, they have already happened.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Event {
    Opened,
    Deposited(i64),
    Withdrawn(i64),
    Closed,
}
// ANCHOR_END: events

// ANCHOR: rejections
/// Why a command may be refused. Not errors: expected business outcomes.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Rejection {
    ExistingAccount,
    NoSuchAccount,
    InsufficientBalance,
    NotSettled,
    AlreadyClosed,
    BadRequest,
}
// ANCHOR_END: rejections

// ANCHOR: account
/// The aggregate root: a small state machine.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Account {
    New,
    Open { balance: i64 },
    Close,
}

impl Account {
    pub fn open(&self) -> Decision<Rejection, Event, i64> {
        let decision = match self {
            Account::New => Decision::accept(Event::Opened),
            _ => Decision::reject(Rejection::ExistingAccount),
        };
        AccountModel // 1
            .perform(self.clone(), decision)
            .validate_with(|s| s.must_be_open()) // 2
    }

    pub fn close(&self) -> Decision<Rejection, Event, Account> {
        let decision = self
            .must_be_open()
            .map_or_else(Decision::Rejected, |balance| {
                // 3
                if balance == 0 {
                    Decision::accept(Event::Closed)
                } else {
                    Decision::reject(Rejection::NotSettled)
                }
            });
        AccountModel.perform(self.clone(), decision)
    }

    pub fn withdraw(&self, amount: i64) -> Decision<Rejection, Event, i64> {
        let decision = self
            .must_be_open()
            .map_or_else(Decision::Rejected, |balance| {
                if balance >= amount && amount > 0 {
                    Decision::accept(Event::Withdrawn(amount))
                } else {
                    Decision::reject(Rejection::InsufficientBalance)
                }
            });
        AccountModel
            .perform(self.clone(), decision)
            .validate_with(|s| s.must_be_open())
    }

    pub fn deposit(&self, amount: i64) -> Decision<Rejection, Event, i64> {
        let decision = self.must_be_open().map_or_else(Decision::Rejected, |_| {
            if amount > 0 {
                Decision::accept(Event::Deposited(amount))
            } else {
                Decision::reject(Rejection::BadRequest)
            }
        });
        AccountModel
            .perform(self.clone(), decision)
            .validate_with(|s| s.must_be_open())
    }

    // 4
    /// A reusable validation (`ValidatedNec` in Scala).
    fn must_be_open(&self) -> Result<i64, NonEmpty<Rejection>> {
        match self {
            Account::Open { balance } => Ok(*balance),
            Account::New => Err(NonEmpty::new(Rejection::NoSuchAccount)),
            Account::Close => Err(NonEmpty::new(Rejection::AlreadyClosed)),
        }
    }
}
// ANCHOR_END: account

// ANCHOR: model
/// The domain model: the initial state and the fold.
#[derive(Clone, Copy, Debug, Default)]
pub struct AccountModel;

impl DomainModel for AccountModel {
    type State = Account;
    type Event = Event;
    type Rejection = Rejection;

    fn initial(&self) -> Account {
        Account::New // 1
    }

    fn transition(&self, event: &Event, state: Account) -> Result<Account, NonEmpty<Rejection>> {
        // 2
        match event {
            Event::Opened => Ok(Account::Open { balance: 0 }),
            Event::Withdrawn(amount) => state
                .must_be_open() // 3
                .map(|balance| Account::Open {
                    balance: balance - amount,
                }),
            Event::Deposited(amount) => state.must_be_open().map(|balance| Account::Open {
                balance: balance + amount,
            }),
            Event::Closed => Ok(Account::Close),
        }
    }
}
// ANCHOR_END: model

// ANCHOR: commands
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Command {
    Open,
    Deposit(i64),
    Withdraw(i64),
    Close,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Notification {
    AccountOpened { account_id: String },
    BalanceUpdated { account_id: String, balance: i64 },
    AccountClosed { account_id: String },
}
// ANCHOR_END: commands

// ANCHOR: service
/// The program type of our service: an `Edomaton` over a `RequestContext`.
pub type AccountApp = App<Command, Account, Event, Rejection, Notification, ()>;

pub fn account_service() -> AccountApp {
    let dsl = AccountModel.dsl::<Command, Notification>();
    dsl.router(move |command| match command {
        Command::Open => dsl
            .state()
            .and_then(move |account| dsl.decide(account.open()))
            .and_then(move |_| dsl.aggregate_id())
            .and_then(move |account_id| dsl.publish([Notification::AccountOpened { account_id }])),

        Command::Deposit(amount) => dsl
            .state()
            .and_then(move |account| dsl.decide(account.deposit(amount)))
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
            .and_then(move |account| dsl.decide(account.withdraw(amount)))
            .and_then(move |balance| {
                dsl.aggregate_id().and_then(move |account_id| {
                    dsl.publish([Notification::BalanceUpdated {
                        account_id,
                        balance,
                    }])
                })
            }),

        Command::Close => dsl
            .state()
            .and_then(move |account| dsl.decide(account.close()))
            .void(),
    })
}
// ANCHOR_END: service

/// Running the service on a request context.
pub async fn run_scenario() -> EdomatonResult<Account, Event, Rejection, Notification> {
    // ANCHOR: scenario
    let scenario1 = RequestContext::new(
        CommandMessage::new(
            "some random id for request",
            chrono::DateTime::<chrono::Utc>::MIN_UTC,
            "our account id",
            Command::Open,
        ),
        Account::New,
    );

    // `execute` folds the decision into the model and yields an `EdomatonResult`.
    let obtained = account_service()
        .execute(&AccountModel, scenario1.clone())
        .await;

    // `run` yields the raw response (decision and notifications).
    let raw: ResponseD<Rejection, Event, Notification, ()> = account_service().run(scenario1).await;
    // ANCHOR_END: scenario
    assert!(raw.result.is_accepted());
    obtained
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decision_examples() {
        decisions();
    }

    #[test]
    fn domain_model_is_testable_without_anything_else() {
        // ANCHOR: model_tests
        assert!(Account::New.open().is_accepted());
        assert_eq!(Account::Open { balance: 10 }.deposit(2).result(), Some(&12));
        assert!(Account::Open { balance: 5 }.close().is_rejected());
        assert!(
            Account::New
                .open()
                .and_then(|_| Account::Open { balance: 0 }.close())
                .is_accepted()
        );
        // ANCHOR_END: model_tests
    }

    #[test]
    fn service_scenario() {
        let obtained = futures::executor::block_on(run_scenario());
        // ANCHOR: scenario_assert
        assert_eq!(
            obtained,
            EdomatonResult::Accepted {
                new_state: Account::Open { balance: 0 },
                events: nonempty![Event::Opened],
                notifications: vec![Notification::AccountOpened {
                    account_id: "our account id".to_string()
                }],
            }
        );
        // ANCHOR_END: scenario_assert
    }
}
