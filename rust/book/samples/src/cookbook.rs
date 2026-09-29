//! Samples of the "Cookbook" chapter. Every recipe runs on the in-memory
//! driver, so the tests of this module exercise them without a database.

use std::time::Duration;

use edomata_backend::eventsourcing::{Backend, PersistedSnapshotConfig};
use edomata_backend::inmemory::InMemoryDriver;
use edomata_backend::{BackendError, CommandResult, RetryConfig};
use edomata_core::*;
use serde::{Deserialize, Serialize};

// ANCHOR: domain
/// What happened to a stock item.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum StockEvent {
    Received { quantity: u32 },
    Shipped { quantity: u32 },
}

/// Why a command is refused.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum StockRejection {
    ZeroQuantity,
    NotEnoughStock { available: u32 },
}

/// The aggregate state: the quantity on hand.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Stock {
    pub on_hand: u32,
}

/// The model: where every aggregate starts, and how events change it.
#[derive(Clone, Copy, Debug, Default)]
pub struct StockModel;

impl DomainModel for StockModel {
    type State = Stock;
    type Event = StockEvent;
    type Rejection = StockRejection;

    fn initial(&self) -> Stock {
        Stock::default()
    }

    fn transition(
        &self,
        event: &StockEvent,
        state: Stock,
    ) -> Result<Stock, NonEmpty<StockRejection>> {
        match event {
            StockEvent::Received { quantity } => Ok(Stock {
                on_hand: state.on_hand.saturating_add(*quantity),
            }),
            // A journal that ships more than it holds is a conflict.
            StockEvent::Shipped { quantity } => state
                .on_hand
                .checked_sub(*quantity)
                .map(|on_hand| Stock { on_hand })
                .ok_or_else(|| {
                    NonEmpty::new(StockRejection::NotEnoughStock {
                        available: state.on_hand,
                    })
                }),
        }
    }
}
// ANCHOR_END: domain

// ANCHOR: validate
/// A reusable validation: a `Result` whose error side is never empty.
pub fn positive(quantity: u32) -> Result<u32, NonEmpty<StockRejection>> {
    if quantity == 0 {
        Err(NonEmpty::new(StockRejection::ZeroQuantity))
    } else {
        Ok(quantity)
    }
}

impl Stock {
    /// Accepts an event, or rejects with a reason.
    pub fn receive(&self, quantity: u32) -> Decision<StockRejection, StockEvent, ()> {
        positive(quantity).map_or_else(Decision::Rejected, |quantity| {
            Decision::accept(StockEvent::Received { quantity })
        })
    }

    /// Validations compose: the first failing one rejects.
    pub fn ship(&self, quantity: u32) -> Decision<StockRejection, StockEvent, ()> {
        let available = self.on_hand;
        positive(quantity).map_or_else(Decision::Rejected, |quantity| {
            if quantity <= available {
                Decision::accept(StockEvent::Shipped { quantity })
            } else {
                Decision::reject(StockRejection::NotEnoughStock { available })
            }
        })
    }
}
// ANCHOR_END: validate

// ANCHOR: commands
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum StockCommand {
    Receive {
        quantity: u32,
    },
    Ship {
        quantity: u32,
        order: String,
    },
    /// Publishes the current level without changing anything.
    Report,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum StockNotification {
    Shipped { item: String, order: String },
    ShipmentRefused { order: String },
    LowStock { item: String, on_hand: u32 },
    Level { item: String, on_hand: u32 },
}

/// The program type of the stock service.
pub type StockApp<T> = App<StockCommand, Stock, StockEvent, StockRejection, StockNotification, T>;

/// Below this level a `LowStock` notification is published.
pub const LOW_STOCK: u32 = 5;
// ANCHOR_END: commands

// ANCHOR: publish
/// Decides, folds the decision into the state, then publishes from the
/// *new* state (`dsl.state()` would give the state before the command).
pub fn ship(quantity: u32, order: String) -> StockApp<()> {
    let dsl = StockModel.dsl::<StockCommand, StockNotification>();
    let refused = [StockNotification::ShipmentRefused {
        order: order.clone(),
    }];
    dsl.state()
        .and_then(move |stock| dsl.decide(StockModel.perform(stock.clone(), stock.ship(quantity))))
        .and_then(move |stock| {
            let order = order.clone();
            dsl.aggregate_id().and_then(move |item| {
                let mut notifications = vec![StockNotification::Shipped {
                    item: item.clone(),
                    order: order.clone(),
                }];
                if stock.on_hand < LOW_STOCK {
                    notifications.push(StockNotification::LowStock {
                        item,
                        on_hand: stock.on_hand,
                    });
                }
                dsl.publish(notifications)
            })
        })
        // Notifications of a rejected command are still written to the outbox.
        .publish_on_rejection(refused)
}
// ANCHOR_END: publish

// ANCHOR: read_state
/// Reads the state and the aggregate id, and publishes; no event is
/// accepted, so the program is indecisive and the journal is unchanged.
pub fn report() -> StockApp<()> {
    let dsl = StockModel.dsl::<StockCommand, StockNotification>();
    dsl.state().and_then(move |stock| {
        dsl.aggregate_id().and_then(move |item| {
            dsl.publish([StockNotification::Level {
                item,
                on_hand: stock.on_hand,
            }])
        })
    })
}
// ANCHOR_END: read_state

// ANCHOR: compose
/// Programs are values: small functions compose into the service.
pub fn receive(quantity: u32) -> StockApp<()> {
    let dsl = StockModel.dsl::<StockCommand, StockNotification>();
    dsl.state()
        .and_then(move |stock| dsl.decide(stock.receive(quantity)))
}

pub fn stock_service() -> StockApp<()> {
    let dsl = StockModel.dsl::<StockCommand, StockNotification>();
    dsl.router(|command| match command {
        StockCommand::Receive { quantity } => receive(quantity),
        StockCommand::Ship { quantity, order } => ship(quantity, order),
        StockCommand::Report => report(),
    })
}

/// `then` sequences two programs: their events and notifications
/// accumulate in order, and a rejection stops the chain.
pub fn receive_and_report(quantity: u32) -> StockApp<()> {
    receive(quantity).then(report())
}
// ANCHOR_END: compose

// ANCHOR: compose_decisions
/// Every program of a chain reads the state of the request context, which a
/// run never changes (`report` above publishes the level from before the
/// receipt). To act on the state produced by earlier events, compose the
/// decisions and fold each one into the state with `perform`.
pub fn restock_and_ship(quantity: u32, order: String) -> StockApp<()> {
    let dsl = StockModel.dsl::<StockCommand, StockNotification>();
    dsl.state()
        .and_then(move |stock| {
            let decision = StockModel
                .perform(stock.clone(), stock.receive(quantity))
                .and_then(|restocked| {
                    StockModel.perform(restocked.clone(), restocked.ship(quantity))
                });
            dsl.decide(decision)
        })
        .and_then(move |_| {
            let order = order.clone();
            dsl.aggregate_id().and_then(move |item| {
                dsl.publish([StockNotification::Shipped {
                    item,
                    order: order.clone(),
                }])
            })
        })
}
// ANCHOR_END: compose_decisions

/// A backend on the in-memory driver.
pub async fn in_memory_backend()
-> Result<Backend<Stock, StockEvent, StockRejection, StockNotification>, BackendError> {
    // ANCHOR: retries
    let backend = Backend::builder(
        StockModel,
        StockModel.dsl::<StockCommand, StockNotification>(),
    )
    .driver(InMemoryDriver::new())
    // Up to 3 attempts; the delay doubles after each conflict, plus jitter.
    .with_retry_config(RetryConfig {
        max_retry: 3,
        initial_delay: Duration::from_millis(50),
    })
    .build_default()
    .await?;
    // ANCHOR_END: retries
    Ok(backend)
}

// ANCHOR: handle_result
/// What a caller does with the outcome of a command.
pub fn describe(result: CommandResult<StockRejection>) -> String {
    match result {
        Ok(Ok(())) => "accepted (or already handled)".to_string(),
        Ok(Err(reasons)) => format!("rejected: {:?}", reasons.as_slice()),
        // Every attempt hit a concurrent write: safe to retry later with the
        // same command id.
        Err(BackendError::MaxRetryExceeded) => "busy, try again".to_string(),
        Err(other) => format!("storage failure: {other}"),
    }
}
// ANCHOR_END: handle_result

// ANCHOR: idempotency
/// A command id derived from the message that caused the command, so a
/// redelivered message produces the same command id.
pub fn shipment_command(order_id: &str, item: &str, quantity: u32) -> CommandMessage<StockCommand> {
    CommandMessage::new(
        format!("ship:{order_id}:{item}"),
        chrono::Utc::now(),
        item,
        StockCommand::Ship {
            quantity,
            order: order_id.to_string(),
        },
    )
}
// ANCHOR_END: idempotency

/// A backend with persisted snapshots and bigger caches.
pub async fn tuned_backend()
-> Result<Backend<Stock, StockEvent, StockRejection, StockNotification>, BackendError> {
    // ANCHOR: snapshots
    let backend = Backend::builder(
        StockModel,
        StockModel.dsl::<StockCommand, StockNotification>(),
    )
    .driver(InMemoryDriver::new())
    // Keep 10 000 aggregates in memory; persist evicted snapshots in
    // batches of 500 or every 30 seconds, and everything on `close`.
    // With `SqlxDriver`, the first argument is a `SqlxCodec<Stock>`.
    .persisted_snapshot_with(
        (),
        PersistedSnapshotConfig {
            size: 10_000,
            max_buffer: 500,
            max_wait: Duration::from_secs(30),
            flush_on_exit: true,
        },
    )
    // Remember the last 10 000 command ids in memory (the `commands`
    // table stays the source of truth).
    .with_command_cache_size(10_000)
    .build_default()
    .await?;
    // ANCHOR_END: snapshots
    Ok(backend)
}

#[cfg(test)]
mod tests {
    use super::*;
    use edomata_testkit::EdomatonAssertions;
    use futures::TryStreamExt;

    #[tokio::test]
    async fn validation_rejects_and_publishes_the_refusal() {
        assert_eq!(
            Stock { on_hand: 3 }.ship(2).events().map(|e| e.len()),
            Some(1)
        );
        assert_eq!(
            Stock { on_hand: 3 }
                .ship(0)
                .rejections()
                .map(|r| r.as_slice().to_vec()),
            Some(vec![StockRejection::ZeroQuantity])
        );
        stock_service()
            .expect_rejection_and_notify(
                &StockModel,
                StockCommand::Ship {
                    quantity: 9,
                    order: "o-1".to_string(),
                },
                Stock { on_hand: 3 },
                [StockRejection::NotEnoughStock { available: 3 }],
                [StockNotification::ShipmentRefused {
                    order: "o-1".to_string(),
                }],
            )
            .await;
    }

    #[tokio::test]
    async fn notifications_come_from_the_new_state() {
        stock_service()
            .expect(
                &StockModel,
                StockCommand::Ship {
                    quantity: 6,
                    order: "o-2".to_string(),
                },
                Stock { on_hand: 10 },
                Stock { on_hand: 4 },
                [
                    StockNotification::Shipped {
                        item: "sut".to_string(),
                        order: "o-2".to_string(),
                    },
                    StockNotification::LowStock {
                        item: "sut".to_string(),
                        on_hand: 4,
                    },
                ],
            )
            .await;
    }

    #[tokio::test]
    async fn reading_state_is_indecisive() -> Result<(), BackendError> {
        let backend = in_memory_backend().await?;
        let service = backend.compile(stock_service());
        // ANCHOR: read_state_outside
        let cmd = CommandMessage::new(
            "r-1",
            chrono::Utc::now(),
            "sku-1",
            StockCommand::Receive { quantity: 8 },
        );
        assert_eq!(service(cmd).await?, Ok(()));
        // From outside a program: the repository folds the journal.
        let state = backend.repository().get("sku-1").await?;
        assert_eq!(state.as_valid().map(|v| v.state.on_hand), Some(8));
        // ANCHOR_END: read_state_outside
        let report = CommandMessage::new("r-2", chrono::Utc::now(), "sku-1", StockCommand::Report);
        assert_eq!(service(report).await?, Ok(()));
        // The report accepted no event: the journal still holds one.
        let events: Vec<_> = backend.journal().read_all().try_collect().await?;
        assert_eq!(events.len(), 1);
        Ok(())
    }

    #[tokio::test]
    async fn composed_programs_accumulate_events() {
        // Both programs see the initial state: the report shows 0.
        let chained = receive_and_report(3)
            .run_with(&StockModel, StockCommand::Report, Stock::default())
            .await;
        assert_eq!(
            chained,
            EdomatonResult::Accepted {
                new_state: Stock { on_hand: 3 },
                events: nonempty![StockEvent::Received { quantity: 3 }],
                notifications: vec![StockNotification::Level {
                    item: "sut".to_string(),
                    on_hand: 0
                }],
            }
        );
        // Composed decisions see each other's state.
        let result = restock_and_ship(3, "o-3".to_string())
            .run_with(&StockModel, StockCommand::Report, Stock::default())
            .await;
        assert_eq!(
            result,
            EdomatonResult::Accepted {
                new_state: Stock { on_hand: 0 },
                events: nonempty![
                    StockEvent::Received { quantity: 3 },
                    StockEvent::Shipped { quantity: 3 }
                ],
                notifications: vec![StockNotification::Shipped {
                    item: "sut".to_string(),
                    order: "o-3".to_string()
                }],
            }
        );
    }

    #[tokio::test]
    async fn the_same_command_is_handled_once() -> Result<(), BackendError> {
        let backend = in_memory_backend().await?;
        let service = backend.compile(stock_service());
        let receipt = CommandMessage::new(
            "receipt-1",
            chrono::Utc::now(),
            "sku-1",
            StockCommand::Receive { quantity: 10 },
        );
        assert_eq!(service(receipt).await?, Ok(()));
        // ANCHOR: idempotency_test
        let command = shipment_command("order-7", "sku-1", 4);
        assert_eq!(service(command.clone()).await?, Ok(()));
        // Redelivered: recognised by its id, nothing is written again.
        assert_eq!(service(command).await?, Ok(()));
        let stock = backend.repository().get("sku-1").await?;
        assert_eq!(stock.as_valid().map(|v| v.state.on_hand), Some(6));
        // ANCHOR_END: idempotency_test
        let outbox: Vec<_> = backend.outbox().read().try_collect().await?;
        assert_eq!(outbox.len(), 1);
        Ok(())
    }

    #[tokio::test]
    async fn retry_gives_up_with_max_retry_exceeded() {
        // ANCHOR: retry_helper
        use edomata_backend::retry;
        use std::sync::atomic::{AtomicU32, Ordering};

        // Your own read-decide-write code can reuse the backend's policy.
        let attempts = AtomicU32::new(0);
        let outcome = retry(3, Duration::from_millis(1), || async {
            if attempts.fetch_add(1, Ordering::SeqCst) < 1 {
                Err(BackendError::VersionConflict) // someone else wrote first
            } else {
                Ok("written")
            }
        })
        .await;
        assert_eq!(outcome, Ok("written"));

        // Still conflicting after the last attempt: `MaxRetryExceeded`.
        let always = retry(2, Duration::from_millis(1), || async {
            Err::<(), _>(BackendError::VersionConflict)
        })
        .await;
        assert_eq!(always, Err(BackendError::MaxRetryExceeded));
        // ANCHOR_END: retry_helper
        assert_eq!(
            describe(Err(BackendError::MaxRetryExceeded)),
            "busy, try again"
        );
        assert_eq!(describe(Ok(Ok(()))), "accepted (or already handled)");
    }

    #[tokio::test]
    async fn snapshots_are_flushed_on_close() -> Result<(), BackendError> {
        let backend = tuned_backend().await?;
        let service = backend.compile(stock_service());
        let cmd = CommandMessage::new(
            "c-1",
            chrono::Utc::now(),
            "sku-9",
            StockCommand::Receive { quantity: 1 },
        );
        assert_eq!(service(cmd).await?, Ok(()));
        backend.close().await
    }
}
