//! Port of `PersistenceSuite.scala`, `BackendCompatibilitySuite.scala` and
//! `SnapshotPersistenceSuite.scala`.

use std::sync::{Arc, Mutex};
use std::time::Duration;

use chrono::{DateTime, Utc};
use edomata_backend::eventsourcing::{
    Backend, NotificationsConsumer, SnapshotPersistence, ValidState,
};
use edomata_backend::{BackendError, EventMessage, EventMetadata, OutboxItem};
use edomata_core::{
    CommandMessage, Edomaton, MessageMetadata, NonEmpty, RequestContext, ResponseD,
};
use futures::{StreamExt, TryStreamExt};
use uuid::Uuid;

use crate::random_string;

/// The event-sourced backend type every suite runs against.
pub type EsBackend = Backend<i32, i32, String, i32>;

type App<T> = Edomaton<RequestContext<String, i32>, String, i32, i32, T>;

fn some_cmd() -> CommandMessage<String> {
    CommandMessage::new(
        random_string(),
        DateTime::UNIX_EPOCH,
        random_string(),
        "command".to_string(),
    )
}

async fn journal_of(s: &EsBackend, address: &str) -> Vec<i32> {
    s.journal()
        .read_stream(address)
        .map_ok(|e| e.payload)
        .try_collect()
        .await
        .unwrap()
}

async fn outbox_of(s: &EsBackend, address: &str) -> Vec<i32> {
    s.outbox()
        .read()
        .try_filter(|i| std::future::ready(i.stream_id == address))
        .map_ok(|i| i.data)
        .try_collect()
        .await
        .unwrap()
}

/// Waits for one update signal, failing after a short timeout.
async fn assert_notified(mut stream: futures::stream::BoxStream<'static, ()>, what: &str) {
    tokio::time::timeout(Duration::from_secs(5), stream.next())
        .await
        .unwrap_or_else(|_| panic!("{what} listeners were not notified"))
        .unwrap_or_else(|| panic!("{what} signal ended"));
}

async fn assert_notified_journal(s: &EsBackend) {
    assert_notified(s.updates().journal(), "journal").await;
}

async fn assert_notified_outbox(s: &EsBackend) {
    assert_notified(s.updates().outbox(), "outbox").await;
}

fn accept_and_publish() -> App<()> {
    Edomaton::lift(ResponseD::accept(NonEmpty::of(1, [2, 3])).publish([4, 5, 6]))
}

// --- PersistenceSuite ------------------------------------------------------

/// "Must append correctly"
pub async fn must_append_correctly(s: &EsBackend) {
    let cmd = some_cmd();
    s.compile(accept_and_publish())(cmd.clone())
        .await
        .unwrap()
        .unwrap();
    assert_eq!(journal_of(s, &cmd.address).await, vec![1, 2, 3]);
    assert_eq!(outbox_of(s, &cmd.address).await, vec![4, 5, 6]);
    assert_notified_journal(s).await;
    assert_notified_outbox(s).await;
}

/// "Appending must be idempotent"
pub async fn appending_must_be_idempotent(s: &EsBackend) {
    let cmd = some_cmd();
    let service = s.compile(accept_and_publish());
    let attempts = futures::future::join_all((0..20).map(|_| service(cmd.clone()))).await;
    for outcome in attempts {
        match outcome {
            Ok(_) | Err(BackendError::VersionConflict) => {}
            Err(other) => panic!("Invalid implementation: {other}"),
        }
    }
    assert_eq!(journal_of(s, &cmd.address).await, vec![1, 2, 3]);
    assert_eq!(outbox_of(s, &cmd.address).await, vec![4, 5, 6]);
    assert_notified_journal(s).await;
    assert_notified_outbox(s).await;
}

/// "Must notify correctly"
pub async fn must_notify_correctly(s: &EsBackend) {
    let cmd = some_cmd();
    let app: App<()> = Edomaton::lift(ResponseD::publish_only([4, 5, 6]));
    s.compile(app)(cmd.clone()).await.unwrap().unwrap();
    assert!(journal_of(s, &cmd.address).await.is_empty());
    assert_eq!(outbox_of(s, &cmd.address).await, vec![4, 5, 6]);
    assert_notified_outbox(s).await;
}

/// "Must consume outbox correctly"
pub async fn must_consume_outbox_correctly(s: &EsBackend) {
    let cmd = some_cmd();
    let app: App<()> = Edomaton::lift(ResponseD::publish_only([4, 5, 6]));
    s.compile(app)(cmd.clone()).await.unwrap().unwrap();
    let items: Vec<OutboxItem<i32>> = s.outbox().read().try_collect().await.unwrap();
    assert!(items.len() >= 3);
    let consumed = items[0].clone();
    s.outbox().mark_as_sent(&consumed).await.unwrap();
    let items2: Vec<OutboxItem<i32>> = s.outbox().read().try_collect().await.unwrap();
    assert!(items2.len() >= 2);
    assert_eq!(items2, items[1..].to_vec());
    let consumed_all = NonEmpty::from_vec(items2.clone()).unwrap();
    s.outbox().mark_all_as_sent(&consumed_all).await.unwrap();
    let items3: Vec<OutboxItem<i32>> = s.outbox().read().try_collect().await.unwrap();
    assert!(
        !items3.windows(items2.len()).any(|w| w == items2.as_slice()),
        "marked items must not be read again"
    );
}

/// "Must read all journal"
pub async fn must_read_all_journal(s: &EsBackend) {
    let cmd = some_cmd();
    let app: App<()> = Edomaton::lift(ResponseD::accept(NonEmpty::of(1, [2, 3])));
    s.compile(app)(cmd).await.unwrap().unwrap();
    let events: Vec<EventMessage<i32>> =
        s.journal().read_all().take(10).try_collect().await.unwrap();
    let ev_size = events.len();
    assert!(ev_size >= 3);
    let mut sorted = events.clone();
    sorted.sort_by_key(|e| e.metadata.seq_nr);
    assert_eq!(events, sorted);
    let pivot_idx = ev_size / 2;
    let pivot = events[pivot_idx].metadata.seq_nr;
    let before = &events[..pivot_idx];
    let after = &events[pivot_idx..];
    let read_before: Vec<EventMessage<i32>> = s
        .journal()
        .read_all_before(pivot)
        .take(before.len())
        .try_collect()
        .await
        .unwrap();
    assert_eq!(read_before, before);
    let read_after: Vec<EventMessage<i32>> = s
        .journal()
        .read_all_after(pivot - 1)
        .take(after.len())
        .try_collect()
        .await
        .unwrap();
    assert_eq!(read_after, after);
}

/// "Must read single stream from journal"
pub async fn must_read_single_stream_from_journal(s: &EsBackend) {
    let cmd = some_cmd();
    let app: App<()> = Edomaton::lift(ResponseD::accept(NonEmpty::of(1, [2, 3])));
    s.compile(app)(cmd.clone()).await.unwrap().unwrap();
    let events: Vec<EventMessage<i32>> = s
        .journal()
        .read_stream(&cmd.address)
        .take(10)
        .try_collect()
        .await
        .unwrap();
    assert_eq!(events.len(), 3);
    let mut by_seq = events.clone();
    by_seq.sort_by_key(|e| e.metadata.seq_nr);
    let mut by_version = events.clone();
    by_version.sort_by_key(|e| e.metadata.version);
    assert_eq!(events, by_seq);
    assert_eq!(events, by_version);
    assert!(events.iter().all(|e| e.metadata.stream == cmd.address));
    let pivot_idx = 1;
    let pivot = events[pivot_idx].metadata.version;
    let before = &events[..pivot_idx];
    let after = &events[pivot_idx..];
    let read_before: Vec<EventMessage<i32>> = s
        .journal()
        .read_stream_before(&cmd.address, pivot)
        .take(before.len())
        .try_collect()
        .await
        .unwrap();
    assert_eq!(read_before, before);
    let read_after: Vec<EventMessage<i32>> = s
        .journal()
        .read_stream_after(&cmd.address, pivot - 1)
        .take(after.len())
        .try_collect()
        .await
        .unwrap();
    assert_eq!(read_after, after);
}

// --- BackendCompatibilitySuite ---------------------------------------------

/// Port of `PreparedData`: rows every compatible storage must contain
/// before the compatibility checks run.
pub mod prepared_data {
    use super::*;

    /// Stream of the prepared aggregate.
    pub const STREAM_ID: &str = "a";
    /// Command id already recorded in the storage.
    pub const REDUNDANT_CMD: &str = "redundant";

    /// The one journaled event.
    pub fn journal() -> Vec<EventMessage<i32>> {
        vec![EventMessage {
            metadata: EventMetadata {
                id: Uuid::nil(),
                time: DateTime::UNIX_EPOCH,
                seq_nr: 1,
                version: 0,
                stream: STREAM_ID.to_string(),
            },
            payload: 1234,
        }]
    }

    /// The one outbox item.
    pub fn outbox() -> Vec<OutboxItem<i32>> {
        vec![OutboxItem {
            seq_nr: 1,
            stream_id: STREAM_ID.to_string(),
            time: DateTime::UNIX_EPOCH,
            data: 123456,
            metadata: MessageMetadata::new("correlation", "causation"),
        }]
    }

    /// The persisted snapshot of the aggregate.
    pub fn aggregate() -> ValidState<i32> {
        ValidState::new(12, 1)
    }

    fn redundant_command() -> CommandMessage<()> {
        CommandMessage::new(REDUNDANT_CMD, DateTime::<Utc>::MIN_UTC, STREAM_ID, ())
    }

    /// "Must read all journal"
    pub async fn must_read_all_journal(s: &EsBackend) {
        let all: Vec<EventMessage<i32>> = s.journal().read_all().try_collect().await.unwrap();
        assert_eq!(all, journal());
    }

    /// "Must read all outbox items"
    pub async fn must_read_all_outbox_items(s: &EsBackend) {
        let expected = outbox();
        let items: Vec<OutboxItem<i32>> = s
            .outbox()
            .read()
            .take(expected.len())
            .try_collect()
            .await
            .unwrap();
        assert_eq!(items, expected);
    }

    /// "Must load for non existing command id"
    pub async fn must_load_for_non_existing_command_id(s: &EsBackend) {
        let mut new_command = redundant_command();
        new_command.id = format!("new-{REDUNDANT_CMD}");
        let seen: Arc<Mutex<Option<RequestContext<(), i32>>>> = Arc::new(Mutex::new(None));
        let sink = Arc::clone(&seen);
        let app: Edomaton<RequestContext<(), i32>, String, i32, i32, ()> =
            Edomaton::run_with(move |ctx: RequestContext<(), i32>| {
                let sink = Arc::clone(&sink);
                async move {
                    *sink.lock().unwrap() = Some(ctx);
                }
            });
        s.compile(app)(new_command.clone()).await.unwrap().unwrap();
        let ctx = seen.lock().unwrap().clone();
        assert_eq!(ctx, Some(RequestContext::new(new_command, 12)));
    }

    /// "Must skip loading for existing command id"
    pub async fn must_skip_loading_for_existing_command_id(s: &EsBackend) {
        let counter = Arc::new(Mutex::new(0));
        let c = Arc::clone(&counter);
        let app: Edomaton<RequestContext<(), i32>, String, i32, i32, ()> =
            Edomaton::eval(move || {
                let c = Arc::clone(&c);
                async move {
                    *c.lock().unwrap() += 1;
                }
            });
        s.compile(app)(redundant_command()).await.unwrap().unwrap();
        assert_eq!(*counter.lock().unwrap(), 0);
    }
}

// --- SnapshotPersistenceSuite ----------------------------------------------

/// "Must read what's written single"
pub async fn snapshot_must_read_whats_written_single(s: &dyn SnapshotPersistence<i32>) {
    let id = random_string();
    let state = ValidState::new(1, 1);
    s.put(vec![(id.clone(), state.clone())]).await.unwrap();
    assert_eq!(s.get(&id).await.unwrap(), Some(state));
}

/// "Must read what's written chunk"
pub async fn snapshot_must_read_whats_written_chunk(s: &dyn SnapshotPersistence<i32>) {
    let id1 = random_string();
    let state1 = ValidState::new(1, 1);
    let id2 = random_string();
    let state2 = ValidState::new(1, 1);
    s.put(vec![
        (id1.clone(), state1.clone()),
        (id2.clone(), state2.clone()),
    ])
    .await
    .unwrap();
    assert_eq!(s.get(&id1).await.unwrap(), Some(state1));
    assert_eq!(s.get(&id2).await.unwrap(), Some(state2));
}

/// "Must deduplicate write chunk"
pub async fn snapshot_must_deduplicate_write_chunk(s: &dyn SnapshotPersistence<i32>) {
    let id = random_string();
    let state = ValidState::new(1, 1);
    s.put(vec![
        (id.clone(), state.clone()),
        (id.clone(), state.clone()),
    ])
    .await
    .unwrap();
    assert_eq!(s.get(&id).await.unwrap(), Some(state));
}

/// "Must write latest items in chunk"
pub async fn snapshot_must_write_latest_items_in_chunk(s: &dyn SnapshotPersistence<i32>) {
    let id = random_string();
    let state1 = ValidState::new(1, 1);
    let state2 = ValidState::new(3, 2);
    s.put(vec![
        (id.clone(), state1.clone()),
        (id.clone(), state2.clone()),
        (id.clone(), state1),
    ])
    .await
    .unwrap();
    assert_eq!(s.get(&id).await.unwrap(), Some(state2));
}

/// Signals consumed by the checks above; exposed so that drivers can reset
/// them between suites if needed.
pub fn updates_of(s: &EsBackend) -> &Arc<dyn NotificationsConsumer> {
    s.updates()
}
