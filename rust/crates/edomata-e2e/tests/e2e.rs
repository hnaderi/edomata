//! Port of `modules/e2e/src/main/scala/e2e.scala` (run by
//! `SkunkE2ETestSuites` and `DoobieE2ETestSuites`): two backends sharing
//! one PostgreSQL namespace, with and without the command/state cache.

use edomata_backend::eventsourcing::{
    AggregateState, Backend, PersistedSnapshotConfig, ValidState,
};
use edomata_backend::{BackendError, DomainService, RetryConfig};
use edomata_core::{CommandMessage, DomainModel};
use edomata_e2e::*;
use edomata_sqlx::{SqlxCodec, SqlxDriver};

type AccountBackend = Backend<Account, Event, Rejection, Notification>;

struct Sut {
    app: AccountBackend,
    service: DomainService<Command, Rejection>,
}

impl Sut {
    async fn open(&self, address: &str) -> Result<(), BackendError> {
        (self.service)(cmd(address, Command::Open))
            .await
            .map(|r| r.unwrap())
    }

    async fn deposit(&self, address: &str, amount: i64) -> Result<(), BackendError> {
        (self.service)(cmd(address, Command::Deposit(Amount::from(amount))))
            .await
            .map(|r| r.unwrap())
    }

    async fn assert_state(&self, address: &str, balance: i64, version: i64) {
        let state = self.app.repository().get(address).await.unwrap();
        assert_eq!(
            state,
            AggregateState::Valid(ValidState {
                state: Account::open_with(Amount::from(balance)),
                version,
            })
        );
    }

    async fn close(self) {
        self.app.close().await.unwrap();
    }
}

fn cmd(address: &str, command: Command) -> CommandMessage<Command> {
    CommandMessage::new(random_string(), chrono::Utc::now(), address, command)
}

async fn sut(cache: bool) -> Sut {
    let driver = SqlxDriver::for_namespace("sqlx_e2e", pool().await)
        .await
        .unwrap();
    let mut builder = Backend::builder(AccountModel, AccountModel.dsl::<Command, Notification>())
        .driver(driver)
        .persisted_snapshot_with(
            SqlxCodec::<Account>::jsonb(),
            PersistedSnapshotConfig {
                size: 100,
                ..PersistedSnapshotConfig::default()
            },
        )
        .with_retry_config(RetryConfig {
            max_retry: 1,
            ..RetryConfig::default()
        });
    if !cache {
        builder = builder.disable_cache();
    }
    let app = builder.build_default().await.unwrap();
    let service = app.compile(account_service());
    Sut { app, service }
}

#[tokio::test]
async fn sanity() {
    let app = sut(false).await;
    let address = random_string();
    app.open(&address).await.unwrap();
    app.assert_state(&address, 0, 1).await;
    app.close().await;
}

#[tokio::test]
async fn distributed_workload_should_work_without_cache() {
    let (app_a, app_b) = (sut(false).await, sut(false).await);
    let address = random_string();
    app_a.open(&address).await.unwrap();
    app_a.deposit(&address, 100).await.unwrap();
    app_b.deposit(&address, 50).await.unwrap();
    app_a.assert_state(&address, 150, 3).await;
    app_b.assert_state(&address, 150, 3).await;
    app_a.close().await;
    app_b.close().await;
}

#[tokio::test]
async fn distributed_workload_does_not_work_with_default_cache() {
    let (app_a, app_b) = (sut(true).await, sut(true).await);
    let address = random_string();
    app_a.open(&address).await.unwrap();
    app_a.deposit(&address, 100).await.unwrap();
    // B doesn't contain the entity yet, so no problem.
    app_b.deposit(&address, 50).await.unwrap();
    app_a.assert_state(&address, 150, 3).await;
    // Now both applications have the entity cached in memory. If one of them
    // changes it, the other's cache becomes stale, so the other application
    // cannot issue a command on that entity any more (version conflict).
    app_b.assert_state(&address, 150, 3).await;
    let conflict = app_a.deposit(&address, 100).await;
    assert!(
        matches!(conflict, Err(BackendError::MaxRetryExceeded)),
        "{conflict:?}"
    );
    app_b.deposit(&address, 50).await.unwrap();
    app_a.assert_state(&address, 200, 4).await;
    app_b.assert_state(&address, 200, 4).await;
    app_a.close().await;
    app_b.close().await;
}

#[tokio::test]
async fn domain_decisions_match_the_scala_model() {
    let account = Account::New {};
    assert!(account.open().is_accepted());
    assert_eq!(
        account
            .deposit(Amount::from(1))
            .rejections()
            .map(|r| r.head()),
        Some(&Rejection::NoSuchAccount)
    );
    let open = Account::open_with(Amount::from(10));
    assert_eq!(
        open.open().rejections().map(|r| r.head()),
        Some(&Rejection::ExistingAccount)
    );
    assert_eq!(
        open.deposit(Amount::from(5)).result(),
        Some(&Amount::from(15))
    );
    assert_eq!(
        open.deposit(Amount::ZERO).rejections().map(|r| r.head()),
        Some(&Rejection::BadRequest)
    );
    assert_eq!(
        open.withdraw(Amount::from(10)).result(),
        Some(&Amount::ZERO)
    );
    assert_eq!(
        open.withdraw(Amount::from(11))
            .rejections()
            .map(|r| r.head()),
        Some(&Rejection::InsufficientBalance)
    );
    assert_eq!(
        open.close().rejections().map(|r| r.head()),
        Some(&Rejection::NotSettled)
    );
    assert_eq!(
        Account::open_with(Amount::ZERO).close().result(),
        Some(&Account::Close {})
    );
    assert_eq!(
        Account::Close {}
            .deposit(Amount::from(1))
            .rejections()
            .map(|r| r.head()),
        Some(&Rejection::AlreadyClosed)
    );
}

#[test]
fn json_shapes_match_circe() {
    assert_eq!(
        serde_json::to_string(&Event::Opened {}).unwrap(),
        r#"{"Opened":{}}"#
    );
    assert_eq!(
        serde_json::to_string(&Event::Deposited {
            amount: Amount::from(100)
        })
        .unwrap(),
        r#"{"Deposited":{"amount":100.0}}"#
    );
    let e: Event = serde_json::from_str(r#"{"Deposited":{"amount":100}}"#).unwrap();
    assert_eq!(
        e,
        Event::Deposited {
            amount: Amount::from(100)
        }
    );
    assert_eq!(
        serde_json::to_string(&Notification::BalanceUpdated {
            account_id: "a".into(),
            balance: Amount::from(150)
        })
        .unwrap(),
        r#"{"BalanceUpdated":{"accountId":"a","balance":150.0}}"#
    );
    let s: Account = serde_json::from_str(r#"{"Open":{"balance":150}}"#).unwrap();
    assert_eq!(s, Account::open_with(Amount::from(150)));
}
