---
sidebar_position: 4
title: "Quickstart"
---

# Quickstart

This page builds a small event-sourced bank account in Rust: the domain logic, a service that
routes commands to it, a PostgreSQL backend that runs it, and unit tests. It is the Rust
counterpart of the Scala [getting started](../tutorials/getting-started.md) tutorial; the
[Rust book](https://beyond-scale-group.github.io/edomata/rust/book/tutorials/getting-started.html)
explains every step in depth.

The code on this page compiles and runs against the `v0.12.34` tag, and its tests pass.

## Dependencies

```toml
[dependencies]
edomata-core = { git = "https://github.com/beyond-scale-group/edomata", tag = "v0.12.34" }
edomata-backend = { git = "https://github.com/beyond-scale-group/edomata", tag = "v0.12.34" }
edomata-sqlx = { git = "https://github.com/beyond-scale-group/edomata", tag = "v0.12.34" }
chrono = { version = "0.4", features = ["clock"] }
serde = { version = "1", features = ["derive"] }
tokio = { version = "1", features = ["rt-multi-thread", "macros"] }

[dev-dependencies]
futures = { version = "0.3", features = ["executor"] }
```

Start PostgreSQL with the repository's `docker-compose.yml` (`docker-compose up -d`), or point
`DATABASE_URL` at any PostgreSQL server.

## Imports

```rust
use edomata_backend::eventsourcing::Backend;
use edomata_core::*;
use edomata_sqlx::{PgPool, SqlxDriver};
use serde::{Deserialize, Serialize};
```

## The domain

Events say what happened, rejections say why a command was refused, and the `DomainModel` folds
events into the state. Business rules are pure functions returning a `Decision`: accepted with
events, or rejected with reasons.

```rust
/// What happened to an account, in the past tense.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Event {
    Opened,
    Deposited(i64),
}

/// Why a command may be refused: expected business outcomes, not errors.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Rejection {
    ExistingAccount,
    NoSuchAccount,
    BadRequest,
}

/// The aggregate state.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Account {
    New,
    Open { balance: i64 },
}

/// The domain model: the initial state and the fold over events.
#[derive(Clone, Copy, Debug, Default)]
pub struct AccountModel;

impl DomainModel for AccountModel {
    type State = Account;
    type Event = Event;
    type Rejection = Rejection;

    fn initial(&self) -> Account {
        Account::New
    }

    fn transition(&self, event: &Event, state: Account) -> Result<Account, NonEmpty<Rejection>> {
        match (event, state) {
            (Event::Opened, _) => Ok(Account::Open { balance: 0 }),
            (Event::Deposited(amount), Account::Open { balance }) => Ok(Account::Open {
                balance: balance + amount,
            }),
            (Event::Deposited(_), Account::New) => Err(NonEmpty::new(Rejection::NoSuchAccount)),
        }
    }
}

/// Business rules: pure functions returning decisions.
impl Account {
    pub fn open(&self) -> Decision<Rejection, Event, ()> {
        match self {
            Account::New => Decision::accept(Event::Opened),
            Account::Open { .. } => Decision::reject(Rejection::ExistingAccount),
        }
    }

    pub fn deposit(&self, amount: i64) -> Decision<Rejection, Event, i64> {
        let decision = match self {
            Account::Open { .. } if amount > 0 => Decision::accept(Event::Deposited(amount)),
            Account::Open { .. } => Decision::reject(Rejection::BadRequest),
            Account::New => Decision::reject(Rejection::NoSuchAccount),
        };
        // `perform` folds the accepted events into the state.
        AccountModel
            .perform(self.clone(), decision)
            .map(|account| match account {
                Account::Open { balance } => balance,
                Account::New => 0,
            })
    }
}
```

## The service

A DSL obtained from the model builds the `Edomaton`: it reads the current state, decides, and
publishes a notification. Notifications are written to the outbox in the same transaction as the
events.

```rust
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Command {
    Open,
    Deposit(i64),
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Notification {
    BalanceUpdated { account_id: String, balance: i64 },
}

/// The service: an `Edomaton` that routes each command to a decision.
pub type AccountApp = App<Command, Account, Event, Rejection, Notification, ()>;

pub fn account_service() -> AccountApp {
    let dsl = AccountModel.dsl::<Command, Notification>();
    dsl.router(move |command| match command {
        Command::Open => dsl
            .state()
            .and_then(move |account| dsl.decide(account.open())),
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
    })
}
```

## Running on PostgreSQL

`SqlxDriver::for_namespace` stores the tables in a PostgreSQL schema and creates them on startup.
`Backend::builder(...).build_default()` uses serde `jsonb` codecs. The compiled service takes a
`CommandMessage` (a unique id, a time, the aggregate id and the payload) and returns `Ok(())`
when the command is accepted or `Err(reasons)` when it is rejected.

```rust
#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let url = std::env::var("DATABASE_URL")
        .unwrap_or_else(|_| "postgres://postgres:postgres@localhost:5432/postgres".to_string());
    let pool = PgPool::connect(&url).await?;

    // The tables live in the PostgreSQL schema `bank`, created on startup.
    let driver = SqlxDriver::for_namespace("bank", pool).await?;
    let backend = Backend::builder(AccountModel, AccountModel.dsl::<Command, Notification>())
        .driver(driver)
        .build_default() // serde `jsonb` codecs for events and notifications
        .await?;

    let service = backend.compile(account_service());
    let now = chrono::Utc::now();
    // `Ok(())` when accepted, `Err(reasons)` when rejected.
    let opened = service(CommandMessage::new("cmd-1", now, "acc-1", Command::Open)).await?;
    let deposited = service(CommandMessage::new(
        "cmd-2",
        now,
        "acc-1",
        Command::Deposit(10),
    ))
    .await?;
    println!("{opened:?} {deposited:?}");

    backend.close().await?;
    Ok(())
}
```

Command ids make commands idempotent: running the program a second time sends the same ids, so
both commands return `Ok(())` without appending any event.

## Testing without a database

An `Edomaton` is a plain value: execute it on a `RequestContext` to test it without any backend.
The tests below use `futures` (with its `executor` feature) as a dev-dependency;
`edomata-testkit` provides assertion helpers for the same purpose.

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deposit_is_accepted() {
        let ctx = RequestContext::new(
            CommandMessage::new(
                "cmd-2",
                chrono::DateTime::<chrono::Utc>::MIN_UTC,
                "account-1",
                Command::Deposit(10),
            ),
            Account::Open { balance: 0 },
        );
        let result = futures::executor::block_on(account_service().execute(&AccountModel, ctx));
        assert_eq!(
            result,
            EdomatonResult::Accepted {
                new_state: Account::Open { balance: 10 },
                events: nonempty![Event::Deposited(10)],
                notifications: vec![Notification::BalanceUpdated {
                    account_id: "account-1".to_string(),
                    balance: 10,
                }],
            }
        );
    }

    #[test]
    fn open_twice_is_rejected() {
        let ctx = RequestContext::new(
            CommandMessage::new(
                "cmd-3",
                chrono::DateTime::<chrono::Utc>::MIN_UTC,
                "account-1",
                Command::Open,
            ),
            Account::Open { balance: 0 },
        );
        let result = futures::executor::block_on(account_service().execute(&AccountModel, ctx));
        assert_eq!(
            result,
            EdomatonResult::Rejected {
                notifications: vec![],
                reasons: nonempty![Rejection::ExistingAccount],
            }
        );
    }
}
```

## Managing the schema with Flyway

With a migration tool, generate the DDL with `PGSchema` and build the driver with `skip_setup`
set to `true`, so that it never runs any DDL itself. `PGNaming::prefixed_str` names the tables
`bank_journal`, `bank_outbox` and so on in the current schema, instead of a dedicated schema.

```rust
use edomata_sqlx::{PgPool, SqlxDriver};

async fn flyway_driver(pool: PgPool) -> Result<SqlxDriver, Box<dyn std::error::Error>> {
    use edomata_sqlx::{PGNaming, PGSchema};

    // Tables named `bank_journal`, `bank_outbox`, ... in the current schema.
    let naming = PGNaming::prefixed_str("bank")?;
    // Paste these statements into a Flyway migration, e.g. `V1__bank.sql`.
    for statement in PGSchema::eventsourcing(&naming) {
        println!("{statement}");
    }
    // `skip_setup = true`: the driver never runs any DDL.
    Ok(SqlxDriver::new_with(naming, pool, true).await?)
}
```

The statements are the same as those produced by the Scala `PGSchema`, so the same migration
serves Scala and Rust services.

## Next

- [Wire compatibility](compatibility.md) with Scala services.
- [The Rust book](https://beyond-scale-group.github.io/edomata/rust/book/): CQRS with `Stomaton`,
  processes, SaaS, event migrations, the Simple API and broker relays.
- [API documentation](https://beyond-scale-group.github.io/edomata/rust/api/edomata_core/).
