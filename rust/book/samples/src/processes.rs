//! Samples of the "Processes" chapter: reading the outbox, the journal and
//! the repository, and relaying to a broker. They need a backend, so they
//! are compiled but not run by the tests.

use std::sync::Arc;

use crate::eventsourcing::{Account, Event, Notification, Rejection};
use edomata_backend::eventsourcing::{AggregateState, Backend};
use edomata_backend::{BackendError, EventMessage, OutboxConsumer, OutboxItem};
use futures::TryStreamExt;

type AccountBackend = Backend<Account, Event, Rejection, Notification>;

// ANCHOR: outbox
/// Consumes outbox items as they are published: runs the handler on every
/// unpublished item, then marks the batch as sent (at-least-once).
pub async fn publisher(backend: &AccountBackend) -> Result<(), BackendError> {
    backend
        .consume_outbox(
            OutboxConsumer::new(),
            |item: OutboxItem<Notification>| async move {
                // use the outboxed item: send to Kafka, call an API, send an email...
                match item.data {
                    Notification::AccountOpened { account_id } => println!("welcome {account_id}"),
                    Notification::BalanceUpdated {
                        account_id,
                        balance,
                    } => {
                        println!("{account_id}: new balance {balance}")
                    }
                    Notification::AccountClosed { .. } => {}
                }
                Ok(())
            },
        )
        .await
}

/// The same, driven by hand: read pending items and mark them.
pub async fn manual_outbox(backend: &AccountBackend) -> Result<(), BackendError> {
    let outbox = backend.outbox();
    let items: Vec<OutboxItem<Notification>> = outbox.read().try_collect().await?;
    for item in &items {
        println!("{} → {:?}", item.stream_id, item.data);
    }
    if let Some(items) = edomata_core::NonEmpty::from_vec(items) {
        outbox.mark_all_as_sent(&items).await?;
    }
    Ok(())
}
// ANCHOR_END: outbox

// ANCHOR: journal
pub async fn journal_reads(backend: &AccountBackend) -> Result<(), BackendError> {
    let journal = backend.journal();
    // Everything, from the beginning.
    let all: Vec<EventMessage<Event>> = journal.read_all().try_collect().await?;
    // One stream (aggregate).
    let single: Vec<EventMessage<Event>> = journal
        .read_stream("interesting-stream")
        .try_collect()
        .await?;
    // Before or after a sequence number / version.
    let all_after: Vec<EventMessage<Event>> = journal.read_all_after(100).try_collect().await?;
    let single_before: Vec<EventMessage<Event>> = journal
        .read_stream_before("interesting-stream", 100)
        .try_collect()
        .await?;
    println!(
        "{} {} {} {}",
        all.len(),
        single.len(),
        all_after.len(),
        single_before.len()
    );
    Ok(())
}
// ANCHOR_END: journal

// ANCHOR: repository
pub async fn repository_reads(backend: &AccountBackend) -> Result<(), BackendError> {
    let repository = backend.repository();
    // Every state the aggregate went through (up to the first conflict, included).
    let history: Vec<AggregateState<Account, Event, Rejection>> = repository
        .history("interesting-stream")
        .try_collect()
        .await?;
    // The current state of the write-side projection.
    let current: AggregateState<Account, Event, Rejection> =
        repository.get("interesting-stream").await?;
    match &current {
        AggregateState::Valid(valid) => println!("{:?} at version {}", valid.state, valid.version),
        AggregateState::Conflicted { .. } => println!("the history conflicts with the model"),
    }
    println!("{} states", history.len());
    Ok(())
}
// ANCHOR_END: repository

// ANCHOR: wakeups
/// A process woken up by the backend's in-process signals.
pub async fn wait_for_outbox(backend: &AccountBackend) {
    use futures::StreamExt;
    let mut signals = backend.updates().outbox(); // or `.journal()`
    signals.next().await; // yields whenever new outbox items were written
}
// ANCHOR_END: wakeups

// ANCHOR: relay
/// Relays the outbox to a broker through `edomata-broker`: items are marked
/// as sent only after the publisher acknowledged them.
pub async fn relay_outbox(backend: &AccountBackend) -> Result<(), edomata_broker::RelayError> {
    use edomata_broker::{
        CancellationToken, MessageEncoder, OutboxRelay, RecordingPublisher, RelayConfig,
    };

    // `RecordingPublisher` stands for `KafkaPublisher` / `RabbitMqPublisher`.
    let publisher = Arc::new(RecordingPublisher::new());
    let relay = OutboxRelay::new(
        Arc::clone(backend.outbox()),
        publisher,
        MessageEncoder::<Notification>::new("application/json", |n| {
            Ok(format!("{n:?}").into_bytes())
        }),
        RelayConfig::new("account"),
    )
    .wake_on(backend.updates().outbox());

    let cancel = CancellationToken::new();
    relay.run(cancel).await
}
// ANCHOR_END: relay
