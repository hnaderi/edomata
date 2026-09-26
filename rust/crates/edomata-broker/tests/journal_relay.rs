//! `JournalRelay` against the in-memory backend and the recording publisher.

mod common;

use std::sync::Arc;
use std::time::Duration;

use common::*;
use edomata_broker::{
    BrokerMessage, CancellationToken, InMemoryCheckpointStore, JournalRelay, MessageEncoder,
    MessageKind, RecordingPublisher, headers,
};

fn make_relay(
    backend: &TestBackend,
    publisher: Arc<RecordingPublisher>,
    checkpoints: Arc<InMemoryCheckpointStore>,
) -> JournalRelay<i32> {
    JournalRelay::new(
        Arc::clone(backend.journal()),
        checkpoints,
        publisher as Arc<_>,
        MessageEncoder::<i32>::serde(),
        config("accounts"),
    )
}

#[tokio::test]
async fn publishes_events_after_the_checkpoint_and_advances_it() {
    let backend = backend().await;
    write(&backend, &["a", "b"], 2).await;
    let publisher = Arc::new(RecordingPublisher::new());
    let checkpoints = Arc::new(InMemoryCheckpointStore::new());
    let relay = make_relay(&backend, Arc::clone(&publisher), Arc::clone(&checkpoints));
    assert_eq!(relay.name(), "accounts:journal");

    assert_eq!(relay.relay_once().await.unwrap(), 4);
    let messages = publisher.messages();
    assert_eq!(messages.len(), 4);
    let seqs: Vec<i64> = messages.iter().map(|m| m.seq_nr).collect();
    let mut sorted = seqs.clone();
    sorted.sort_unstable();
    assert_eq!(seqs, sorted);
    assert_eq!(
        checkpoints.get("accounts:journal"),
        Some(*seqs.last().unwrap())
    );
    for m in &messages {
        assert_eq!(m.kind, MessageKind::Event);
        assert_eq!(m.id, BrokerMessage::journal_id("accounts", m.seq_nr));
        assert!(m.extra_headers.contains_key(headers::EVENT_ID));
        assert!(m.extra_headers.contains_key(headers::VERSION));
        assert_eq!(m.content_type, "application/json");
    }
    let payloads: Vec<String> = messages.iter().map(|m| m.payload_text()).collect();
    assert_eq!(payloads, vec!["1", "1", "2", "2"]);

    // Nothing new: nothing published, checkpoint unchanged.
    assert_eq!(relay.relay_once().await.unwrap(), 0);
    assert_eq!(publisher.messages().len(), 4);

    // New events only.
    write(&backend, &["c"], 1).await;
    assert_eq!(relay.relay_once().await.unwrap(), 1);
    assert_eq!(publisher.messages().last().unwrap().stream_id, "c");
    assert_eq!(checkpoints.get("accounts:journal"), Some(5));
}

#[tokio::test]
async fn a_new_relay_resumes_from_the_stored_checkpoint() {
    let backend = backend().await;
    write(&backend, &["a"], 3).await;
    let checkpoints = Arc::new(InMemoryCheckpointStore::new());
    let first = Arc::new(RecordingPublisher::new());
    make_relay(&backend, Arc::clone(&first), Arc::clone(&checkpoints))
        .relay_once()
        .await
        .unwrap();
    write(&backend, &["a"], 2).await;
    let second = Arc::new(RecordingPublisher::new());
    let relay = make_relay(&backend, Arc::clone(&second), checkpoints);
    assert_eq!(relay.relay_once().await.unwrap(), 2);
    let seqs: Vec<i64> = second.messages().iter().map(|m| m.seq_nr).collect();
    assert_eq!(seqs, vec![4, 5], "only the events after the checkpoint");
}

#[tokio::test]
async fn checkpoint_does_not_move_when_publishing_fails() {
    let backend = backend().await;
    write(&backend, &["a"], 2).await;
    let publisher = Arc::new(RecordingPublisher::new());
    publisher.fail_next(100);
    let checkpoints = Arc::new(InMemoryCheckpointStore::new());
    let relay = make_relay(&backend, Arc::clone(&publisher), Arc::clone(&checkpoints));
    assert!(relay.relay_once().await.is_err());
    assert_eq!(checkpoints.get("accounts:journal"), None);
    // Crash between publishing and checkpointing: redelivered with the same ids.
    let publisher = Arc::new(RecordingPublisher::new());
    publisher.fail_after_publishing_next(1);
    let relay = make_relay(&backend, Arc::clone(&publisher), Arc::clone(&checkpoints));
    assert_eq!(relay.relay_once().await.unwrap(), 2);
    let ids = publisher.ids();
    assert_eq!(ids.len(), 4);
    assert_eq!(&ids[..2], &ids[2..]);
}

#[tokio::test]
async fn named_relays_keep_separate_checkpoints_and_run_wakes_on_journal_signal() {
    let backend = backend().await;
    let checkpoints = Arc::new(InMemoryCheckpointStore::new());
    let publisher = Arc::new(RecordingPublisher::new());
    let relay = Arc::new(
        make_relay(&backend, Arc::clone(&publisher), Arc::clone(&checkpoints))
            .with_name("audit")
            .wake_on(backend.updates().journal()),
    );
    assert_eq!(relay.name(), "audit");
    let cancel = CancellationToken::new();
    let running = tokio::spawn({
        let relay = Arc::clone(&relay);
        let cancel = cancel.clone();
        async move { relay.run(cancel).await }
    });
    tokio::time::sleep(Duration::from_millis(20)).await;
    write(&backend, &["a"], 2).await;
    tokio::time::timeout(Duration::from_secs(5), async {
        while publisher.messages().len() < 2 {
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .unwrap();
    cancel.cancel();
    running.await.unwrap().unwrap();
    assert_eq!(checkpoints.get("audit"), Some(2));
    assert_eq!(checkpoints.get("accounts:journal"), None);
}
