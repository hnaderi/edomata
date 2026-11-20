//! Pulsar integration tests (testcontainers, `apachepulsar/pulsar` standalone;
//! Docker required): acknowledgment before marking, redelivery after a crash
//! between publishing and marking, per-stream ordering, deduplication by
//! message id, full topic URLs and producer recreation after `close`.
//!
//! Not covered here: token authentication and TLS (the standalone image runs
//! without either) and the Clever Cloud add-on.

use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::{Duration, Instant};

use async_trait::async_trait;
use edomata_backend::OutboxReader;
use edomata_backend::eventsourcing::Backend;
use edomata_backend::inmemory::InMemoryDriver;
use edomata_broker::{
    BrokerMessage, MessageEncoder, OutboxRelay, PublishError, Publisher, RelayConfig, RetryPolicy,
    headers,
};
use edomata_core::*;
use edomata_pulsar::PulsarPublisher;
use edomata_pulsar::pulsar::consumer::InitialPosition;
use edomata_pulsar::pulsar::message::proto::command_subscribe::SubType;
use edomata_pulsar::pulsar::{ConsumerOptions, Pulsar, TokioExecutor};
use futures::TryStreamExt;
use serde::{Deserialize, Serialize};
use testcontainers::ContainerAsync;
use testcontainers::runners::AsyncRunner;
use testcontainers_modules::pulsar;

// ---------------------------------------------------------------------------
// Fixtures
// ---------------------------------------------------------------------------

struct Broker {
    _container: ContainerAsync<pulsar::Pulsar>,
    service_url: String,
}

static BROKER: tokio::sync::OnceCell<Broker> = tokio::sync::OnceCell::const_new();

/// The image reports "messaging service is ready" before the standalone
/// broker created the `public/default` namespace: a producer on it fails
/// with "Namespace not found" until then. Waits until one can be created.
async fn wait_for_default_namespace(service_url: &str) {
    let deadline = Instant::now() + Duration::from_secs(90);
    loop {
        let attempt = async {
            let client = Pulsar::builder(service_url, TokioExecutor).build().await?;
            client
                .producer()
                .with_topic("persistent://public/default/edomata-readiness-probe")
                .build()
                .await
                .map(|_| ())
        };
        match tokio::time::timeout(Duration::from_secs(10), attempt).await {
            Ok(Ok(())) => return,
            outcome => {
                assert!(
                    Instant::now() < deadline,
                    "public/default never became usable: {outcome:?}"
                );
                tokio::time::sleep(Duration::from_secs(1)).await;
            }
        }
    }
}

async fn service_url() -> String {
    BROKER
        .get_or_init(|| async {
            let container = pulsar::Pulsar::default()
                .start()
                .await
                .expect("Docker must be available to start the Pulsar container");
            let port = container.get_host_port_ipv4(6650).await.unwrap();
            let service_url = format!("pulsar://127.0.0.1:{port}");
            wait_for_default_namespace(&service_url).await;
            Broker {
                _container: container,
                service_url,
            }
        })
        .await
        .service_url
        .clone()
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
struct Notif {
    stream: String,
    n: i32,
}

struct Counter;

impl DomainModel for Counter {
    type State = i32;
    type Event = i32;
    type Rejection = String;

    fn initial(&self) -> i32 {
        0
    }

    fn transition(&self, e: &i32, s: i32) -> Result<i32, NonEmpty<String>> {
        Ok(s + e)
    }
}

type TestBackend = Backend<i32, i32, String, Notif>;

fn app() -> App<i32, i32, i32, String, Notif, ()> {
    let dsl = Counter.dsl::<i32, Notif>();
    dsl.router(move |n| {
        dsl.aggregate_id()
            .and_then(move |stream| dsl.accept(n).then(dsl.publish([Notif { stream, n }])))
    })
}

async fn backend() -> TestBackend {
    Backend::builder(Counter, Counter.dsl::<i32, Notif>())
        .driver(InMemoryDriver::new())
        .build_default()
        .await
        .unwrap()
}

async fn write(backend: &TestBackend, streams: &[&str], rounds: i32) {
    let service = backend.compile(app());
    for round in 1..=rounds {
        for stream in streams {
            let cmd = CommandMessage::new(
                uuid::Uuid::new_v4().to_string(),
                chrono::Utc::now(),
                *stream,
                round,
            );
            service(cmd).await.unwrap().unwrap();
        }
    }
}

async fn pending(reader: &dyn OutboxReader<Notif>) -> usize {
    reader.read().try_collect::<Vec<_>>().await.unwrap().len()
}

fn config(source: &str) -> RelayConfig {
    RelayConfig::new(source)
        .with_poll_interval(Duration::from_millis(100))
        .with_leader_retry_interval(Duration::from_millis(100))
        .with_retry(RetryPolicy {
            initial_delay: Duration::from_millis(10),
            max_delay: Duration::from_millis(50),
            max_retries: Some(3),
        })
}

/// A unique short topic name.
fn topic(prefix: &str) -> String {
    format!("{prefix}-{}", uuid::Uuid::new_v4().simple())
}

/// The full name of a short topic in the default namespace.
fn full(topic: &str) -> String {
    format!("persistent://public/default/{topic}")
}

async fn publisher(topic: &str) -> Arc<PulsarPublisher> {
    Arc::new(
        PulsarPublisher::builder(service_url().await)
            .with_fixed_topic(topic)
            .with_send_timeout(Duration::from_secs(20))
            .build()
            .unwrap(),
    )
}

#[derive(Debug, PartialEq, Eq)]
struct Received {
    key: String,
    payload: String,
    properties: BTreeMap<String, String>,
}

/// Consumes `topic` (a full topic name) from the beginning until `expected`
/// messages arrived or `quiet` elapsed without a new one.
async fn consume(topic: &str, expected: usize, quiet: Duration) -> Vec<Received> {
    let client = Pulsar::builder(service_url().await, TokioExecutor)
        .build()
        .await
        .unwrap();
    let mut consumer: edomata_pulsar::pulsar::Consumer<Vec<u8>, _> = client
        .consumer()
        .with_topic(topic)
        .with_subscription(format!("test-{}", uuid::Uuid::new_v4()))
        .with_subscription_type(SubType::Exclusive)
        .with_options(ConsumerOptions {
            initial_position: InitialPosition::Earliest,
            ..Default::default()
        })
        .build()
        .await
        .unwrap();
    let mut out = Vec::new();
    let deadline = Instant::now() + Duration::from_secs(60);
    loop {
        let wait = if out.len() >= expected {
            quiet
        } else {
            Duration::from_secs(20)
        };
        match tokio::time::timeout(wait, consumer.try_next()).await {
            Ok(Ok(Some(message))) => {
                out.push(Received {
                    key: message.key().unwrap_or_default(),
                    payload: String::from_utf8_lossy(&message.payload.data).into_owned(),
                    properties: message
                        .payload
                        .metadata
                        .properties
                        .iter()
                        .map(|kv| (kv.key.clone(), kv.value.clone()))
                        .collect(),
                });
            }
            Ok(Ok(None)) => return out,
            Ok(Err(e)) => panic!("consumer failed: {e}"),
            Err(_) => {
                if out.len() >= expected || Instant::now() > deadline {
                    return out;
                }
            }
        }
    }
}

/// Publishes through `inner`, then reports the given error for the next
/// `count` batches (the broker took the messages, the relay never learnt it).
struct CrashAfterPublish<P> {
    inner: P,
    remaining: AtomicUsize,
}

#[async_trait]
impl<P: Publisher> Publisher for CrashAfterPublish<P> {
    async fn publish(&self, batch: &NonEmpty<BrokerMessage>) -> Result<(), PublishError> {
        self.inner.publish(batch).await?;
        // `fetch_update` is deprecated in favour of `try_update`, newer than the MSRV.
        #[allow(deprecated)]
        let crash = self
            .remaining
            .fetch_update(Ordering::SeqCst, Ordering::SeqCst, |n| n.checked_sub(1))
            .is_ok();
        if crash {
            Err(PublishError::transient("crashed before marking"))
        } else {
            Ok(())
        }
    }
}

/// A publisher pointed at a closed port: connecting fails.
fn unreachable_publisher() -> Arc<PulsarPublisher> {
    Arc::new(
        PulsarPublisher::builder("pulsar://127.0.0.1:1")
            .with_fixed_topic("unreachable")
            .with_send_timeout(Duration::from_secs(3))
            .build()
            .unwrap(),
    )
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn publisher_delivers_keys_properties_payloads_in_order() {
    let topic = topic("direct");
    let publisher = publisher(&topic).await;
    let backend = backend().await;
    write(&backend, &["a", "b"], 3).await;
    let relay = OutboxRelay::new(
        Arc::clone(backend.outbox()),
        publisher.clone() as Arc<_>,
        MessageEncoder::<Notif>::serde(),
        config("accounts"),
    );
    assert_eq!(relay.relay_once().await.unwrap(), 6);
    assert_eq!(pending(backend.outbox().as_ref()).await, 0);

    let received = consume(&full(&topic), 6, Duration::from_secs(2)).await;
    assert_eq!(received.len(), 6);
    let keys: Vec<&str> = received.iter().map(|r| r.key.as_str()).collect();
    assert_eq!(keys, vec!["a", "b", "a", "b", "a", "b"]);
    let first = &received[0];
    assert_eq!(first.payload, r#"{"stream":"a","n":1}"#);
    assert_eq!(first.properties[headers::ID], "accounts:outbox:1");
    assert_eq!(first.properties[headers::SOURCE], "accounts");
    assert_eq!(first.properties[headers::KIND], "notification");
    assert_eq!(first.properties[headers::STREAM], "a");
    assert_eq!(first.properties[headers::SEQ_NR], "1");
    assert_eq!(first.properties[headers::CONTENT_TYPE], "application/json");
    assert!(first.properties.contains_key(headers::TIME));
    assert!(first.properties.contains_key(headers::CORRELATION));
    let seqs: Vec<i64> = received
        .iter()
        .map(|r| r.properties[headers::SEQ_NR].parse().unwrap())
        .collect();
    assert_eq!(seqs, (1..=6).collect::<Vec<_>>());
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn nothing_is_marked_as_sent_before_the_broker_acknowledges() {
    let backend = backend().await;
    write(&backend, &["a"], 3).await;
    let relay = OutboxRelay::new(
        Arc::clone(backend.outbox()),
        unreachable_publisher() as Arc<_>,
        MessageEncoder::<Notif>::serde(),
        config("accounts"),
    );
    assert!(relay.relay_once().await.is_err());
    assert_eq!(pending(backend.outbox().as_ref()).await, 3, "still pending");

    // The broker comes back: everything is delivered exactly once.
    let topic = topic("late");
    let relay = OutboxRelay::new(
        Arc::clone(backend.outbox()),
        publisher(&topic).await as Arc<_>,
        MessageEncoder::<Notif>::serde(),
        config("accounts"),
    );
    assert_eq!(relay.relay_once().await.unwrap(), 3);
    assert_eq!(pending(backend.outbox().as_ref()).await, 0);
    assert_eq!(
        consume(&full(&topic), 3, Duration::from_secs(2))
            .await
            .len(),
        3
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn redelivery_after_a_crash_between_publishing_and_marking_is_deduplicated_by_id() {
    let topic = topic("crash");
    let backend = backend().await;
    write(&backend, &["a", "b"], 2).await;
    let crashing = Arc::new(CrashAfterPublish {
        inner: publisher(&topic).await,
        remaining: AtomicUsize::new(1),
    });
    let relay = OutboxRelay::new(
        Arc::clone(backend.outbox()),
        crashing as Arc<_>,
        MessageEncoder::<Notif>::serde(),
        config("accounts"),
    );
    assert_eq!(relay.relay_once().await.unwrap(), 4);
    assert_eq!(pending(backend.outbox().as_ref()).await, 0);

    let received = consume(&full(&topic), 8, Duration::from_secs(2)).await;
    assert_eq!(received.len(), 8, "delivered twice: at-least-once");
    let ids: BTreeSet<&str> = received
        .iter()
        .map(|r| r.properties[headers::ID].as_str())
        .collect();
    assert_eq!(
        ids.len(),
        4,
        "consumers deduplicate on the stable message id"
    );
    let mut by_id: HashMap<&str, Vec<&Received>> = HashMap::new();
    for r in &received {
        by_id
            .entry(r.properties[headers::ID].as_str())
            .or_default()
            .push(r);
    }
    for copies in by_id.values() {
        assert_eq!(copies.len(), 2);
        assert_eq!(copies[0], copies[1], "identical redelivery");
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn per_stream_ordering_is_preserved() {
    let topic = topic("ordering");
    let backend = backend().await;
    write(&backend, &["x", "y", "z"], 10).await;
    let relay = OutboxRelay::new(
        Arc::clone(backend.outbox()),
        publisher(&topic).await as Arc<_>,
        MessageEncoder::<Notif>::serde(),
        config("accounts").with_batch_size(7),
    );
    assert_eq!(relay.relay_once().await.unwrap(), 30);
    let received = consume(&full(&topic), 30, Duration::from_secs(2)).await;
    assert_eq!(received.len(), 30);
    let mut last: HashMap<&str, i32> = HashMap::new();
    for r in &received {
        let n: Notif = serde_json::from_str(&r.payload).unwrap();
        let prev = last.insert(r.key.as_str(), n.n).unwrap_or(0);
        assert_eq!(n.n, prev + 1, "stream {} out of order", r.key);
    }
    assert_eq!(last.len(), 3);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn full_topic_urls_and_per_source_topics_are_honoured() {
    let url = service_url().await;
    let by_source = format!("src-{}", uuid::Uuid::new_v4().simple());
    let fixed = topic("url");
    let backend = backend().await;
    write(&backend, &["a"], 2).await;

    // Default topic: one per relay source, in the default namespace.
    let per_source = Arc::new(PulsarPublisher::builder(url.clone()).build().unwrap());
    let relay = OutboxRelay::new(
        Arc::clone(backend.outbox()),
        per_source as Arc<_>,
        MessageEncoder::<Notif>::serde(),
        config(&by_source),
    );
    assert_eq!(relay.relay_once().await.unwrap(), 2);
    assert_eq!(
        consume(&full(&by_source), 2, Duration::from_secs(2))
            .await
            .len(),
        2
    );

    // A full topic URL is used as it is.
    write(&backend, &["b"], 2).await;
    let by_url = Arc::new(
        PulsarPublisher::builder(url)
            .with_fixed_topic(full(&fixed))
            .build()
            .unwrap(),
    );
    let relay = OutboxRelay::new(
        Arc::clone(backend.outbox()),
        by_url as Arc<_>,
        MessageEncoder::<Notif>::serde(),
        config(&by_source),
    );
    assert_eq!(relay.relay_once().await.unwrap(), 2);
    assert_eq!(
        consume(&full(&fixed), 2, Duration::from_secs(2))
            .await
            .len(),
        2
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn the_publisher_reconnects_after_close() {
    let topic = topic("reconnect");
    let publisher = publisher(&topic).await;
    let backend = backend().await;
    write(&backend, &["a"], 4).await;
    let relay = OutboxRelay::new(
        Arc::clone(backend.outbox()),
        publisher.clone() as Arc<_>,
        MessageEncoder::<Notif>::serde(),
        config("accounts").with_batch_size(2),
    );
    assert_eq!(relay.relay_once().await.unwrap(), 4);
    publisher.close().await;
    write(&backend, &["a"], 2).await;
    assert_eq!(relay.relay_once().await.unwrap(), 2);
    assert_eq!(
        consume(&full(&topic), 6, Duration::from_secs(2))
            .await
            .len(),
        6
    );
}
