//! `OutboxRelay` against the in-memory backend and the recording publisher.

mod common;

use std::sync::Arc;
use std::time::Duration;

use common::*;
use edomata_broker::{
    BrokerMessage, CancellationToken, MessageKind, OutboxRelay, PublishError, RecordingPublisher,
    RelayError, headers,
};

fn make_relay(backend: &TestBackend, publisher: Arc<RecordingPublisher>) -> OutboxRelay<Notif> {
    OutboxRelay::new(
        Arc::clone(backend.outbox()),
        publisher as Arc<_>,
        encoder(),
        config("accounts"),
    )
}

#[tokio::test]
async fn messages_have_deterministic_ids_headers_and_json_payloads() {
    let backend = backend().await;
    write(&backend, &["s1"], 1).await;
    let publisher = Arc::new(RecordingPublisher::new());
    let relay = make_relay(&backend, Arc::clone(&publisher));
    let item = pending(backend.outbox().as_ref()).await.remove(0);
    let message = relay.message(&item).unwrap();
    assert_eq!(
        message.id,
        BrokerMessage::outbox_id("accounts", item.seq_nr)
    );
    assert_eq!(message.id, format!("accounts:outbox:{}", item.seq_nr));
    assert_eq!(message.kind, MessageKind::Notification);
    assert_eq!(message.stream_id, "s1");
    assert_eq!(message.seq_nr, item.seq_nr);
    assert_eq!(message.time, item.time);
    assert_eq!(message.content_type, "application/json");
    assert_eq!(message.payload_text(), r#"{"stream":"s1","n":1}"#);
    assert_eq!(message.correlation, item.metadata.correlation);
    let hs = message.headers();
    assert_eq!(hs[0], (headers::ID.to_string(), message.id.clone()));
    assert!(hs.contains(&(headers::KIND.to_string(), "notification".to_string())));
    assert!(hs.contains(&(headers::STREAM.to_string(), "s1".to_string())));
    assert!(hs.contains(&(headers::SEQ_NR.to_string(), item.seq_nr.to_string())));
    assert!(hs.iter().any(|(k, _)| k == headers::CORRELATION));
    assert!(hs.iter().any(|(k, _)| k == headers::CAUSATION));
    // Same item, same id: the id is stable across relays.
    assert_eq!(relay.message(&item).unwrap().id, message.id);
}

#[tokio::test]
async fn relay_once_publishes_in_order_and_marks_only_afterwards() {
    let backend = backend().await;
    write(&backend, &["a", "b"], 3).await;
    let publisher = Arc::new(RecordingPublisher::new());
    let relay = make_relay(&backend, Arc::clone(&publisher));
    assert_eq!(pending(backend.outbox().as_ref()).await.len(), 6);

    assert_eq!(relay.relay_once().await.unwrap(), 6);
    let messages = publisher.messages();
    let seqs: Vec<i64> = messages.iter().map(|m| m.seq_nr).collect();
    let mut sorted = seqs.clone();
    sorted.sort_unstable();
    assert_eq!(seqs, sorted, "published in sequence order");
    let streams: Vec<&str> = messages.iter().map(|m| m.stream_id.as_str()).collect();
    assert_eq!(streams, vec!["a", "b", "a", "b", "a", "b"]);
    assert!(
        pending(backend.outbox().as_ref()).await.is_empty(),
        "marked as sent"
    );
    assert_eq!(relay.metrics().published(), 6);
    // Nothing left: a second pass publishes nothing.
    assert_eq!(relay.relay_once().await.unwrap(), 0);
    assert_eq!(publisher.messages().len(), 6);
    assert_eq!(relay.metrics().snapshot().passes, 2);
}

#[tokio::test]
async fn batches_respect_the_batch_size() {
    let backend = backend().await;
    write(&backend, &["a"], 5).await;
    let publisher = Arc::new(RecordingPublisher::new());
    let relay = OutboxRelay::new(
        Arc::clone(backend.outbox()),
        Arc::clone(&publisher) as Arc<_>,
        encoder(),
        config("accounts").with_batch_size(2),
    );
    assert_eq!(relay.relay_once().await.unwrap(), 5);
    assert_eq!(publisher.calls(), 3, "2 + 2 + 1");
}

#[tokio::test]
async fn nothing_is_marked_while_the_broker_is_down_then_retried() {
    let backend = backend().await;
    write(&backend, &["a"], 2).await;
    let publisher = Arc::new(RecordingPublisher::new());
    publisher.fail_next(2);
    let relay = make_relay(&backend, Arc::clone(&publisher));
    assert_eq!(relay.relay_once().await.unwrap(), 2);
    assert_eq!(publisher.calls(), 3);
    assert_eq!(
        publisher.messages().len(),
        2,
        "published exactly once after the broker came back"
    );
    assert_eq!(relay.metrics().snapshot().retried, 2);
    assert!(pending(backend.outbox().as_ref()).await.is_empty());
}

#[tokio::test]
async fn retry_budget_exhausted_leaves_items_pending() {
    let backend = backend().await;
    write(&backend, &["a"], 1).await;
    let publisher = Arc::new(RecordingPublisher::new());
    publisher.fail_next(100);
    let relay = make_relay(&backend, Arc::clone(&publisher));
    let err = relay.relay_once().await.unwrap_err();
    assert!(
        matches!(err, RelayError::Publish(PublishError::Transient(_))),
        "{err}"
    );
    assert_eq!(
        pending(backend.outbox().as_ref()).await.len(),
        1,
        "still pending"
    );
    assert!(publisher.messages().is_empty());
    assert_eq!(relay.metrics().snapshot().failed, 1);
}

#[tokio::test]
async fn crash_between_publishing_and_marking_redelivers_with_the_same_ids() {
    let backend = backend().await;
    write(&backend, &["a", "b"], 2).await;
    let publisher = Arc::new(RecordingPublisher::new());
    // The broker takes the batch but the acknowledgment is lost.
    publisher.fail_after_publishing_next(1);
    let relay = make_relay(&backend, Arc::clone(&publisher));
    assert_eq!(relay.relay_once().await.unwrap(), 4);
    let ids = publisher.ids();
    assert_eq!(
        ids.len(),
        8,
        "the batch was delivered twice (at-least-once)"
    );
    let unique: std::collections::BTreeSet<&String> = ids.iter().collect();
    assert_eq!(unique.len(), 4, "consumers deduplicate on the stable id");
    assert_eq!(&ids[..4], &ids[4..]);
    assert!(pending(backend.outbox().as_ref()).await.is_empty());
}

#[tokio::test]
async fn permanent_failure_stops_the_relay_without_marking() {
    struct Rejecting;
    #[async_trait::async_trait]
    impl edomata_broker::Publisher for Rejecting {
        async fn publish(
            &self,
            _: &edomata_core::NonEmpty<BrokerMessage>,
        ) -> Result<(), PublishError> {
            Err(PublishError::permanent("message too large"))
        }
    }
    let backend = backend().await;
    write(&backend, &["a"], 1).await;
    let relay = OutboxRelay::new(
        Arc::clone(backend.outbox()),
        Arc::new(Rejecting),
        encoder(),
        config("accounts"),
    );
    let err = relay.relay_once().await.unwrap_err();
    assert!(
        matches!(err, RelayError::Publish(PublishError::Permanent(_))),
        "{err}"
    );
    assert_eq!(pending(backend.outbox().as_ref()).await.len(), 1);
    let run = relay.run(CancellationToken::new()).await;
    assert!(run.is_err(), "run stops on a permanent failure");
}

#[tokio::test]
async fn run_is_woken_up_by_the_backend_signal_and_stops_on_cancel() {
    let backend = backend().await;
    let publisher = Arc::new(RecordingPublisher::new());
    let relay = Arc::new(
        OutboxRelay::new(
            Arc::clone(backend.outbox()),
            Arc::clone(&publisher) as Arc<_>,
            encoder(),
            // A long poll interval: only the signal can wake the relay up.
            config("accounts").with_poll_interval(Duration::from_secs(60)),
        )
        .wake_on(backend.updates().outbox()),
    );
    let cancel = CancellationToken::new();
    let running = tokio::spawn({
        let relay = Arc::clone(&relay);
        let cancel = cancel.clone();
        async move { relay.run(cancel).await }
    });
    tokio::time::sleep(Duration::from_millis(50)).await;
    write(&backend, &["a"], 3).await;
    tokio::time::timeout(Duration::from_secs(5), async {
        while publisher.messages().len() < 3 {
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .expect("the relay was woken up by the outbox signal");
    assert!(pending(backend.outbox().as_ref()).await.is_empty());
    cancel.cancel();
    running.await.unwrap().unwrap();
}

#[tokio::test]
async fn run_polls_when_there_is_no_signal() {
    let backend = backend().await;
    let publisher = Arc::new(RecordingPublisher::new());
    let relay = Arc::new(make_relay(&backend, Arc::clone(&publisher)));
    let cancel = CancellationToken::new();
    let running = tokio::spawn({
        let relay = Arc::clone(&relay);
        let cancel = cancel.clone();
        async move { relay.run(cancel).await }
    });
    write(&backend, &["a"], 2).await;
    tokio::time::timeout(Duration::from_secs(5), async {
        while publisher.messages().len() < 2 {
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .expect("the polling fallback relayed the items");
    cancel.cancel();
    running.await.unwrap().unwrap();
}
