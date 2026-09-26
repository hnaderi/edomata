//! RabbitMQ integration tests (testcontainers): acknowledgment before
//! marking, redelivery after a crash between publishing and marking,
//! per-stream ordering, deduplication by message id, and leader election
//! with two relays.

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
use edomata_rabbitmq::RabbitMqPublisher;
use edomata_rabbitmq::lapin::options::{
    BasicAckOptions, BasicConsumeOptions, ExchangeDeclareOptions, QueueBindOptions,
    QueueDeclareOptions,
};
use edomata_rabbitmq::lapin::types::{AMQPValue, FieldTable};
use edomata_rabbitmq::lapin::{Connection, ConnectionProperties, ExchangeKind};
use futures::{StreamExt, TryStreamExt};
use serde::{Deserialize, Serialize};
use testcontainers::ContainerAsync;
use testcontainers::runners::AsyncRunner;
use testcontainers_modules::rabbitmq::RabbitMq;

// ---------------------------------------------------------------------------
// Fixtures
// ---------------------------------------------------------------------------

struct Broker {
    _container: ContainerAsync<RabbitMq>,
    uri: String,
}

static BROKER: tokio::sync::OnceCell<Broker> = tokio::sync::OnceCell::const_new();

async fn uri() -> String {
    BROKER
        .get_or_init(|| async {
            let container = RabbitMq::default()
                .start()
                .await
                .expect("Docker must be available to start the RabbitMQ container");
            let port = container.get_host_port_ipv4(5672).await.unwrap();
            Broker {
                _container: container,
                uri: format!("amqp://guest:guest@127.0.0.1:{port}/%2f"),
            }
        })
        .await
        .uri
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

/// A fanout exchange bound to a fresh queue; returns (exchange, queue).
async fn topology(prefix: &str) -> (String, String) {
    let name = format!("{prefix}-{}", uuid::Uuid::new_v4().simple());
    let conn = Connection::connect(&uri().await, ConnectionProperties::default())
        .await
        .unwrap();
    let channel = conn.create_channel().await.unwrap();
    channel
        .exchange_declare(
            name.as_str().into(),
            ExchangeKind::Fanout,
            ExchangeDeclareOptions {
                durable: true,
                ..Default::default()
            },
            FieldTable::default(),
        )
        .await
        .unwrap();
    channel
        .queue_declare(
            name.as_str().into(),
            QueueDeclareOptions {
                durable: true,
                ..Default::default()
            },
            FieldTable::default(),
        )
        .await
        .unwrap();
    channel
        .queue_bind(
            name.as_str().into(),
            name.as_str().into(),
            "".into(),
            QueueBindOptions::default(),
            FieldTable::default(),
        )
        .await
        .unwrap();
    (name.clone(), name)
}

async fn publisher(exchange: &str) -> Arc<RabbitMqPublisher> {
    Arc::new(
        RabbitMqPublisher::connect(&uri().await)
            .await
            .unwrap()
            .with_fixed_exchange(exchange),
    )
}

#[derive(Debug, PartialEq, Eq)]
struct Received {
    routing_key: String,
    message_id: String,
    delivery_mode: u8,
    content_type: String,
    payload: String,
    headers: BTreeMap<String, String>,
}

/// Consumes `queue` until `expected` messages arrived or `quiet` elapsed
/// without a new message.
async fn consume(queue: &str, expected: usize, quiet: Duration) -> Vec<Received> {
    let conn = Connection::connect(&uri().await, ConnectionProperties::default())
        .await
        .unwrap();
    let channel = conn.create_channel().await.unwrap();
    let mut consumer = channel
        .basic_consume(
            queue.into(),
            "test".into(),
            BasicConsumeOptions::default(),
            FieldTable::default(),
        )
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
        match tokio::time::timeout(wait, consumer.next()).await {
            Ok(Some(Ok(delivery))) => {
                let props = &delivery.properties;
                let mut headers = BTreeMap::new();
                if let Some(table) = props.headers() {
                    for (k, v) in table.inner() {
                        if let AMQPValue::LongString(s) = v {
                            headers.insert(
                                k.to_string(),
                                String::from_utf8_lossy(s.as_bytes()).into_owned(),
                            );
                        }
                    }
                }
                out.push(Received {
                    routing_key: delivery.routing_key.to_string(),
                    message_id: props
                        .message_id()
                        .as_ref()
                        .map(|s| s.to_string())
                        .unwrap_or_default(),
                    delivery_mode: props.delivery_mode().unwrap_or(0),
                    content_type: props
                        .content_type()
                        .as_ref()
                        .map(|s| s.to_string())
                        .unwrap_or_default(),
                    payload: String::from_utf8_lossy(&delivery.data).into_owned(),
                    headers,
                });
                delivery.ack(BasicAckOptions::default()).await.unwrap();
            }
            Ok(Some(Err(e))) => panic!("consumer error: {e}"),
            Ok(None) => return out,
            Err(_) => {
                if out.len() >= expected || Instant::now() > deadline {
                    return out;
                }
            }
        }
    }
}

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

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn publisher_delivers_persistent_messages_with_ids_headers_and_routing_keys() {
    let (exchange, queue) = topology("direct").await;
    let backend = backend(InMemoryDriver::new()).await;
    write(&backend, &["a", "b"], 3).await;
    let relay = OutboxRelay::new(
        Arc::clone(backend.outbox()),
        publisher(&exchange).await as Arc<_>,
        MessageEncoder::<Notif>::serde(),
        config("accounts"),
    );
    assert_eq!(relay.relay_once().await.unwrap(), 6);
    assert_eq!(pending(backend.outbox().as_ref()).await, 0);

    let received = consume(&queue, 6, Duration::from_secs(2)).await;
    assert_eq!(received.len(), 6);
    let keys: Vec<&str> = received.iter().map(|r| r.routing_key.as_str()).collect();
    assert_eq!(keys, vec!["a", "b", "a", "b", "a", "b"]);
    let first = &received[0];
    assert_eq!(first.message_id, "accounts:outbox:1");
    assert_eq!(first.delivery_mode, 2, "persistent");
    assert_eq!(first.content_type, "application/json");
    assert_eq!(first.payload, r#"{"stream":"a","n":1}"#);
    assert_eq!(first.headers[headers::SOURCE], "accounts");
    assert_eq!(first.headers[headers::KIND], "notification");
    assert_eq!(first.headers[headers::STREAM], "a");
    assert_eq!(first.headers[headers::SEQ_NR], "1");
    assert!(first.headers.contains_key(headers::CORRELATION));
    let ids: Vec<&str> = received.iter().map(|r| r.message_id.as_str()).collect();
    assert_eq!(
        ids,
        (1..=6)
            .map(|i| format!("accounts:outbox:{i}"))
            .collect::<Vec<_>>()
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn nothing_is_marked_as_sent_before_the_broker_acknowledges() {
    let backend = backend(InMemoryDriver::new()).await;
    write(&backend, &["a"], 3).await;
    // Publishing to an exchange that does not exist closes the channel: the
    // publish fails and nothing is marked.
    let missing = Arc::new(
        RabbitMqPublisher::connect(&uri().await)
            .await
            .unwrap()
            .with_fixed_exchange(format!("missing-{}", uuid::Uuid::new_v4().simple())),
    );
    let relay = OutboxRelay::new(
        Arc::clone(backend.outbox()),
        missing as Arc<_>,
        MessageEncoder::<Notif>::serde(),
        config("accounts"),
    );
    assert!(relay.relay_once().await.is_err());
    assert_eq!(pending(backend.outbox().as_ref()).await, 3, "still pending");

    let (exchange, queue) = topology("late").await;
    let relay = OutboxRelay::new(
        Arc::clone(backend.outbox()),
        publisher(&exchange).await as Arc<_>,
        MessageEncoder::<Notif>::serde(),
        config("accounts"),
    );
    assert_eq!(relay.relay_once().await.unwrap(), 3);
    assert_eq!(pending(backend.outbox().as_ref()).await, 0);
    assert_eq!(consume(&queue, 3, Duration::from_secs(2)).await.len(), 3);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn redelivery_after_a_crash_between_publishing_and_marking_is_deduplicated_by_id() {
    let (exchange, queue) = topology("crash").await;
    let backend = backend(InMemoryDriver::new()).await;
    write(&backend, &["a", "b"], 2).await;
    let crashing = Arc::new(CrashAfterPublish {
        inner: publisher(&exchange).await,
        remaining: AtomicUsize::new(1),
    });
    let relay = OutboxRelay::new(
        Arc::clone(backend.outbox()),
        crashing as Arc<_>,
        MessageEncoder::<Notif>::serde(),
        config("accounts"),
    );
    assert_eq!(relay.relay_once().await.unwrap(), 4);
    let received = consume(&queue, 8, Duration::from_secs(2)).await;
    assert_eq!(received.len(), 8, "delivered twice: at-least-once");
    let ids: BTreeSet<&str> = received.iter().map(|r| r.message_id.as_str()).collect();
    assert_eq!(ids.len(), 4, "consumers deduplicate on message_id");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn per_stream_ordering_is_preserved() {
    let (exchange, queue) = topology("ordering").await;
    let backend = backend(InMemoryDriver::new()).await;
    write(&backend, &["x", "y", "z"], 10).await;
    let relay = OutboxRelay::new(
        Arc::clone(backend.outbox()),
        publisher(&exchange).await as Arc<_>,
        MessageEncoder::<Notif>::serde(),
        config("accounts").with_batch_size(7),
    );
    assert_eq!(relay.relay_once().await.unwrap(), 30);
    let received = consume(&queue, 30, Duration::from_secs(2)).await;
    assert_eq!(received.len(), 30);
    let mut last: HashMap<&str, i32> = HashMap::new();
    for r in &received {
        let n: Notif = serde_json::from_str(&r.payload).unwrap();
        let prev = last.insert(r.routing_key.as_str(), n.n).unwrap_or(0);
        assert_eq!(n.n, prev + 1, "stream {} out of order", r.routing_key);
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
    sqlx::query("DROP SCHEMA IF EXISTS rabbitmq_leader CASCADE")
        .execute(&pool)
        .await
        .unwrap();
    let driver = edomata_sqlx::SqlxDriver::for_namespace("rabbitmq_leader", pool.clone())
        .await
        .unwrap();
    let backend = backend(driver).await;
    let (exchange, queue) = topology("leader").await;
    let source = format!("rabbitmq_leader-{}", uuid::Uuid::new_v4());
    let cancel = CancellationToken::new();
    let mut runs = Vec::new();
    for _ in 0..2 {
        let relay = Arc::new(OutboxRelay::new(
            Arc::clone(backend.outbox()),
            publisher(&exchange).await as Arc<_>,
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
    let received = consume(&queue, 10, Duration::from_secs(3)).await;
    assert_eq!(received.len(), 10, "exactly one relay published each item");
    let ids: BTreeSet<&str> = received.iter().map(|r| r.message_id.as_str()).collect();
    assert_eq!(ids.len(), 10);
    cancel.cancel();
    for run in runs {
        run.await.unwrap().unwrap();
    }
    backend.close().await.unwrap();
}
