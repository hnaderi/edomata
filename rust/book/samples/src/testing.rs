//! Samples of the "Testing" chapter: domain tests with `edomata-testkit`,
//! service tests on the in-memory driver, and an integration test against
//! PostgreSQL (it reads `DATABASE_URL`, like the other database tests of
//! the workspace).

#[cfg(test)]
mod tests {
    use crate::eventsourcing::{
        Account, AccountModel, Command, Event, Notification, Rejection, account_service,
    };
    use edomata_core::nonempty;
    use edomata_testkit::EdomatonAssertions;

    #[test]
    fn expectations_read_like_the_scala_domain_suite() {
        futures::executor::block_on(async {
            // ANCHOR: testkit
            let app = account_service();
            // command, state → expected new state and notifications (in order)
            app.expect(
                &AccountModel,
                Command::Open,
                Account::New,
                Account::Open { balance: 0 },
                [Notification::AccountOpened {
                    account_id: "sut".to_string(),
                }],
            )
            .await;
            // rejections, and no notification
            app.expect_rejection_with(
                &AccountModel,
                Command::Deposit(-1),
                Account::Open { balance: 0 },
                [Rejection::BadRequest],
            )
            .await;
            let (notifications, reasons) = app
                .expect_rejection(&AccountModel, Command::Close, Account::Open { balance: 5 })
                .await;
            assert!(notifications.is_empty());
            assert_eq!(reasons, nonempty![Rejection::NotSettled]);
            // ANCHOR_END: testkit
        });
    }

    #[tokio::test]
    async fn custom_commands_and_predicates() {
        // ANCHOR: testkit_custom
        use edomata_core::EdomatonResult;
        use edomata_testkit::TestCommand;

        let app = account_service();
        // Assertions on the new state instead of an exact value.
        app.expect_that(
            &AccountModel,
            Command::Deposit(10),
            Account::Open { balance: 5 },
            [Notification::BalanceUpdated {
                account_id: "sut".to_string(),
                balance: 15,
            }],
            |state| assert!(matches!(state, Account::Open { balance } if *balance > 10)),
        )
        .await;

        // A custom message id and aggregate address; the raw result.
        let test = TestCommand::new("cmd-42", "account-7");
        let result = app
            .run_with_command(&AccountModel, &test, Command::Open, Account::New)
            .await;
        assert_eq!(
            result,
            EdomatonResult::Accepted {
                new_state: Account::Open { balance: 0 },
                events: nonempty![Event::Opened],
                notifications: vec![Notification::AccountOpened {
                    account_id: "account-7".to_string()
                }],
            }
        );
        // ANCHOR_END: testkit_custom
    }

    #[tokio::test]
    async fn stomaton_expectations() {
        // ANCHOR: testkit_cqrs
        use crate::cqrs::{self, Order, OrderStatus, order_service};
        use edomata_testkit::StomatonAssertions;

        let app = order_service();
        app.expect(
            cqrs::Command::Place {
                food: "taco".to_string(),
                address: "home".to_string(),
            },
            Order::Empty,
            Order::Placed {
                food: "taco".to_string(),
                address: "home".to_string(),
                status: OrderStatus::New,
            },
            [cqrs::Notification::Received {
                food: "taco".to_string(),
            }],
        )
        .await;
        app.expect_rejection_with(
            cqrs::Command::MarkAsCooking {
                cook: "chef".to_string(),
            },
            Order::Empty,
            [cqrs::Rejection::InvalidRequest],
            [],
        )
        .await;
        // ANCHOR_END: testkit_cqrs
    }

    // ANCHOR: in_memory
    /// The whole service (command handling, idempotency, outbox, journal)
    /// on the in-memory driver: no database, same semantics as PostgreSQL.
    #[tokio::test]
    async fn service_on_the_in_memory_driver() -> Result<(), edomata_backend::BackendError> {
        use edomata_backend::eventsourcing::Backend;
        use edomata_backend::inmemory::InMemoryDriver;
        use edomata_core::{CommandMessage, DomainModel};
        use futures::TryStreamExt;

        let backend = Backend::builder(AccountModel, AccountModel.dsl::<Command, Notification>())
            .driver(InMemoryDriver::new())
            .build_default()
            .await?;
        let service = backend.compile(account_service());
        let cmd = |id: &str, command| CommandMessage::new(id, chrono::Utc::now(), "acc-1", command);

        assert_eq!(service(cmd("c1", Command::Open)).await?, Ok(()));
        assert_eq!(service(cmd("c2", Command::Deposit(50))).await?, Ok(()));
        assert_eq!(
            service(cmd("c3", Command::Withdraw(80))).await?,
            Err(nonempty![Rejection::InsufficientBalance])
        );

        let events: Vec<Event> = backend
            .journal()
            .read_stream("acc-1")
            .map_ok(|e| e.payload)
            .try_collect()
            .await?;
        assert_eq!(events, vec![Event::Opened, Event::Deposited(50)]);
        let state = backend.repository().get("acc-1").await?;
        assert_eq!(
            state.as_valid().map(|v| (v.state.clone(), v.version)),
            Some((Account::Open { balance: 50 }, 2))
        );
        let outbox: Vec<_> = backend.outbox().read().try_collect().await?;
        assert_eq!(outbox.len(), 2);
        Ok(())
    }
    // ANCHOR_END: in_memory

    // ANCHOR: postgres
    /// An integration test against a real PostgreSQL: `DATABASE_URL`, or the
    /// docker-compose instance by default. Each test uses its own prefixed
    /// namespace and drops its tables first, so reruns start clean.
    #[tokio::test]
    async fn account_lifecycle_on_postgres() -> Result<(), Box<dyn std::error::Error>> {
        use edomata_backend::eventsourcing::Backend;
        use edomata_core::{CommandMessage, DomainModel};
        use edomata_sqlx::{PGNaming, SqlxCodec, SqlxDriver};
        use futures::TryStreamExt;

        let url = std::env::var("DATABASE_URL")
            .unwrap_or_else(|_| "postgres://postgres:postgres@localhost:5432/postgres".into());
        let pool = sqlx::postgres::PgPoolOptions::new()
            .max_connections(4)
            .connect(&url)
            .await?;
        let prefix = "book_testing_accounts";
        for table in ["journal", "outbox", "commands", "snapshots"] {
            sqlx::query(&format!("DROP TABLE IF EXISTS {prefix}_{table}"))
                .execute(&pool)
                .await?;
        }

        let driver = SqlxDriver::new(PGNaming::prefixed_str(prefix)?, pool.clone()).await?;
        let backend = Backend::builder(AccountModel, AccountModel.dsl::<Command, Notification>())
            .driver(driver)
            .persisted_snapshot(SqlxCodec::<Account>::jsonb())
            .build_default()
            .await?;
        let service = backend.compile(account_service());
        let cmd = |id: &str, command| CommandMessage::new(id, chrono::Utc::now(), "acc-1", command);

        assert_eq!(service(cmd("c1", Command::Open)).await?, Ok(()));
        assert_eq!(service(cmd("c2", Command::Deposit(30))).await?, Ok(()));
        // The same command id again is redundant: nothing new is written.
        assert_eq!(service(cmd("c2", Command::Deposit(30))).await?, Ok(()));

        let journal: Vec<_> = backend.journal().read_stream("acc-1").try_collect().await?;
        assert_eq!(journal.len(), 2);
        let rows: i64 = sqlx::query_scalar(&format!("SELECT count(*) FROM {prefix}_outbox"))
            .fetch_one(&pool)
            .await?;
        assert_eq!(rows, 2);

        backend.close().await?; // flushes the persisted snapshots
        let snapshots: i64 =
            sqlx::query_scalar(&format!("SELECT count(*) FROM {prefix}_snapshots"))
                .fetch_one(&pool)
                .await?;
        assert_eq!(snapshots, 1);
        Ok(())
    }
    // ANCHOR_END: postgres
}
