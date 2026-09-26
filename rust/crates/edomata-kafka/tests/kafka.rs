//! Kafka integration tests (testcontainers): acknowledgment before marking,
//! redelivery after a crash between publishing and marking, per-stream
//! ordering, deduplication by message id, and leader election with two
//! relays.

use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::{Duration, Instant};

use async_trait::async_trait;
use edomata_backend::OutboxReader;
use edomata_backend::eventsourcing::Backend;
use edomata_backend::inmemory::InMemoryDriver;
use edomata_broker::postgres::LeaderLock;
use edomata_broker::{
    BrokerMessage, CancellationToken, MessageEncoder, OutboxRelay, PublishError, Publisher,
    RelayConfig, RetryPolicy, headers,
};
use edomata_core::*;
use edomata_kafka::KafkaPublisher;
use edomata_kafka::rdkafka::consumer::{Consumer, StreamConsumer};
use edomata_kafka::rdkafka::message::Headers;
use edomata_kafka::rdkafka::{ClientConfig, Message};
use futures::TryStreamExt;
use serde::{Deserialize, Serialize};
use testcontainers::ContainerAsync;
use testcontainers::runners::AsyncRunner;
use testcontainers_modules::kafka::apache;

// ---------------------------------------------------------------------------
// Fixtures
// ---------------------------------------------------------------------------

struct Broker {
    _container: ContainerAsync<apache::Kafka>,
    bootstrap: String,
}

static BROKER: tokio::sync::OnceCell<Broker> = tokio::sync::OnceCell::const_new();

async fn bootstrap() -> String {
    BROKER
        .get_or_init(|| async {
            let container = apache::Kafka::default()
                .start()
                .await
                .expect("Docker must be available to start the Kafka container");
            let port = container
                .get_host_port_ipv4(apache::KAFKA_PORT)
                .await
                .unwrap();
            Broker {
                _container: container,
                bootstrap: format!("127.0.0.1:{port}"),
            }
        })
        .await
        .bootstrap
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

async fn backend<D>(driver: D) -> TestBackend
where
    D: edomata_backend::eventsourcing::StorageDriver,
    D::Codec<i32>: Default,
    D::Codec<Notif>: Default,
{
    Backend::builder(Counter, Counter.dsl::<i32, Notif>())
        .driver(driver)
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

fn topic(prefix: &str) -> String {
    format!("{prefix}-{}", uuid::Uuid::new_v4().simple())
}

async fn publisher(topic: &str) -> Arc<KafkaPublisher> {
    Arc::new(
        KafkaPublisher::builder(&bootstrap().await)
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
    headers: BTreeMap<String, String>,
}

/// Consumes from the beginning of `topic` until `expected` messages arrived
/// or `quiet` elapsed without a new message.
async fn consume(topic: &str, expected: usize, quiet: Duration) -> Vec<Received> {
    let consumer: StreamConsumer = ClientConfig::new()
        .set("bootstrap.servers", bootstrap().await)
        .set("group.id", format!("test-{}", uuid::Uuid::new_v4()))
        .set("auto.offset.reset", "earliest")
        .set("enable.auto.commit", "false")
        .create()
        .unwrap();
    consumer.subscribe(&[topic]).unwrap();
    let mut out = Vec::new();
    let deadline = Instant::now() + Duration::from_secs(60);
    loop {
        let wait = if out.len() >= expected {
            quiet
        } else {
            Duration::from_secs(20)
        };
        match tokio::time::timeout(wait, consumer.recv()).await {
            Ok(Ok(message)) => {
                let mut headers = BTreeMap::new();
                if let Some(hs) = message.headers() {
                    for h in hs.iter() {
                        headers.insert(
                            h.key.to_string(),
                            String::from_utf8_lossy(h.value.unwrap_or_default()).into_owned(),
                        );
                    }
                }
                out.push(Received {
                    key: String::from_utf8_lossy(message.key().unwrap_or_default()).into_owned(),
                    payload: String::from_utf8_lossy(message.payload().unwrap_or_default())
                        .into_owned(),
                    headers,
                });
            }
            // Subscribing before the relay created the topic yields
            // UnknownTopicOrPartition until the metadata refreshes: keep
            // waiting (the deadline bounds the wait).
            Ok(Err(e)) => {
                eprintln!("consumer error (retrying): {e}");
                tokio::time::sleep(Duration::from_millis(200)).await;
                if Instant::now() > deadline {
                    return out;
                }
            }
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

/// A real producer pointed at a closed port: deliveries time out.
fn unreachable_publisher() -> Arc<KafkaPublisher> {
    Arc::new(
        KafkaPublisher::builder("127.0.0.1:1")
            .with_config("message.timeout.ms", "1500")
            .with_config("socket.connection.setup.timeout.ms", "1000")
            .with_fixed_topic("unreachable")
            .with_send_timeout(Duration::from_secs(5))
            .build()
            .unwrap(),
    )
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn publisher_delivers_keys_headers_payloads_in_order() {
    let topic = topic("direct");
    let publisher = publisher(&topic).await;
    let backend = backend(InMemoryDriver::new()).await;
    write(&backend, &["a", "b"], 3).await;
    let relay = OutboxRelay::new(
        Arc::clone(backend.outbox()),
        publisher.clone() as Arc<_>,
        MessageEncoder::<Notif>::serde(),
        config("accounts"),
    );
    assert_eq!(relay.relay_once().await.unwrap(), 6);
    assert_eq!(pending(backend.outbox().as_ref()).await, 0);

    let received = consume(&topic, 6, Duration::from_secs(2)).await;
    assert_eq!(received.len(), 6);
    let keys: Vec<&str> = received.iter().map(|r| r.key.as_str()).collect();
    assert_eq!(keys, vec!["a", "b", "a", "b", "a", "b"]);
    let first = &received[0];
    assert_eq!(first.payload, r#"{"stream":"a","n":1}"#);
    assert_eq!(first.headers[headers::ID], "accounts:outbox:1");
    assert_eq!(first.headers[headers::SOURCE], "accounts");
    assert_eq!(first.headers[headers::KIND], "notification");
    assert_eq!(first.headers[headers::STREAM], "a");
    assert_eq!(first.headers[headers::SEQ_NR], "1");
    assert_eq!(first.headers[headers::CONTENT_TYPE], "application/json");
    assert!(first.headers.contains_key(headers::TIME));
    assert!(first.headers.contains_key(headers::CORRELATION));
    let seqs: Vec<i64> = received
        .iter()
        .map(|r| r.headers[headers::SEQ_NR].parse().unwrap())
        .collect();
    assert_eq!(seqs, (1..=6).collect::<Vec<_>>());
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn nothing_is_marked_as_sent_before_the_broker_acknowledges() {
    let backend = backend(InMemoryDriver::new()).await;
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
    assert_eq!(consume(&topic, 3, Duration::from_secs(2)).await.len(), 3);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn redelivery_after_a_crash_between_publishing_and_marking_is_deduplicated_by_id() {
    let topic = topic("crash");
    let backend = backend(InMemoryDriver::new()).await;
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

    let received = consume(&topic, 8, Duration::from_secs(2)).await;
    assert_eq!(received.len(), 8, "delivered twice: at-least-once");
    let ids: BTreeSet<&str> = received
        .iter()
        .map(|r| r.headers[headers::ID].as_str())
        .collect();
    assert_eq!(
        ids.len(),
        4,
        "consumers deduplicate on the stable message id"
    );
    let mut by_id: HashMap<&str, Vec<&Received>> = HashMap::new();
    for r in &received {
        by_id
            .entry(r.headers[headers::ID].as_str())
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
    let backend = backend(InMemoryDriver::new()).await;
    write(&backend, &["x", "y", "z"], 10).await;
    let relay = OutboxRelay::new(
        Arc::clone(backend.outbox()),
        publisher(&topic).await as Arc<_>,
        MessageEncoder::<Notif>::serde(),
        config("accounts").with_batch_size(7),
    );
    assert_eq!(relay.relay_once().await.unwrap(), 30);
    let received = consume(&topic, 30, Duration::from_secs(2)).await;
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
async fn two_relays_publish_each_item_once_thanks_to_leader_election() {
    let url = std::env::var("DATABASE_URL")
        .unwrap_or_else(|_| "postgres://postgres:postgres@localhost:5432/postgres".to_string());
    let pool = sqlx::postgres::PgPoolOptions::new()
        .max_connections(8)
        .connect(&url)
        .await
        .expect("PostgreSQL from docker-compose must be running (see rust/README.md)");
    sqlx::query("DROP SCHEMA IF EXISTS kafka_leader CASCADE")
        .execute(&pool)
        .await
        .unwrap();
    let driver = edomata_sqlx::SqlxDriver::for_namespace("kafka_leader", pool.clone())
        .await
        .unwrap();
    let backend = backend(driver).await;
    let topic = topic("leader");
    let source = format!("kafka_leader-{}", uuid::Uuid::new_v4());
    let cancel = CancellationToken::new();
    let mut runs = Vec::new();
    for _ in 0..2 {
        let relay = Arc::new(OutboxRelay::new(
            Arc::clone(backend.outbox()),
            publisher(&topic).await as Arc<_>,
            MessageEncoder::<Notif>::serde(),
            config(&source),
        ));
        let (cancel, pool, source) = (cancel.clone(), pool.clone(), source.clone());
        runs.push(tokio::spawn(async move {
            relay
                .run_as_leader(LeaderLock::new(pool, &source), cancel)
                .await
        }));
    }
    tokio::time::sleep(Duration::from_millis(500)).await;
    write(&backend, &["a", "b"], 5).await;
    let received = consume(&topic, 10, Duration::from_secs(3)).await;
    assert_eq!(received.len(), 10, "exactly one relay published each item");
    let ids: BTreeSet<&str> = received
        .iter()
        .map(|r| r.headers[headers::ID].as_str())
        .collect();
    assert_eq!(ids.len(), 10);
    cancel.cancel();
    for run in runs {
        run.await.unwrap().unwrap();
    }
    backend.close().await.unwrap();
}
