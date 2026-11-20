//! # Edomata Pulsar
//!
//! A [`Publisher`] for `edomata-broker` relays backed by the
//! [`pulsar`] client (Apache Pulsar, including the managed add-on of Clever
//! Cloud):
//!
//! - one Pulsar producer per topic, created lazily and cached; a producer
//!   (or the whole connection) that fails is dropped so that the next
//!   `publish` recreates it;
//! - the stream id is the message key, which keeps per-stream ordering (also
//!   on partitioned topics: the producer routes by key);
//! - the topic is chosen by a function of the message (default: one topic
//!   per relay source, i.e. per aggregate namespace); short names are placed
//!   in the configured tenant and namespace, full
//!   `persistent://<tenant>/<namespace>/<topic>` URLs are used as they are;
//! - the message id and metadata travel as message properties (see
//!   `edomata_broker::headers`).
//!
//! `publish` returns only after the broker acknowledged every message of the
//! batch (a send receipt each); the relay marks outbox items as sent after
//! that, so delivery is at-least-once and consumers deduplicate on the
//! `edomata-id` property.
//!
//! Errors are classified for the relay: invalid topics, authentication and
//! authorization failures, rejected topics and messages larger than the
//! configured maximum are [`PublishError::Permanent`]; everything else
//! (connection lost, timeouts, broker not ready...) is
//! [`PublishError::Transient`] and retried. When unsure the error is
//! transient.
//!
//! ## Where it fits
//!
//! `edomata-pulsar` depends only on `edomata-core` and `edomata-broker`: it
//! is one [`Publisher`] implementation for the broker-agnostic
//! [`OutboxRelay`](edomata_broker::OutboxRelay) and
//! [`JournalRelay`](edomata_broker::JournalRelay). Applications that do not
//! use Pulsar don't depend on it. It has no Scala counterpart. The
//! [`pulsar`] crate is re-exported for its configuration and error types.
//!
//! ## Limits of a shared namespace
//!
//! A Pulsar producer belongs to one topic, so the number of distinct topics
//! your topic function produces is the number of producers this publisher
//! keeps. Managed offerings cap them: Clever Cloud documents at most 100
//! topics per namespace and 100 producers per topic. Prefer
//! [`with_fixed_topic`](PulsarPublisherBuilder::with_fixed_topic) or a
//! function with a bounded range; never derive the topic from a stream id.
//!
//! ## Example
//!
//! An outbox relay publishing to one Pulsar topic with token authentication
//! over TLS (the connection is made at the first publish, not at build time):
//!
//! ```no_run
//! use std::sync::Arc;
//!
//! use edomata_backend::eventsourcing::Backend;
//! use edomata_broker::postgres::LeaderLock;
//! use edomata_broker::{CancellationToken, MessageEncoder, OutboxRelay, RelayConfig};
//! use edomata_core::*;
//! use edomata_pulsar::PulsarPublisher;
//! use edomata_sqlx::SqlxDriver;
//!
//! struct Counter;
//! impl DomainModel for Counter {
//!     type State = i32; type Event = i32; type Rejection = String;
//!     fn initial(&self) -> i32 { 0 }
//!     fn transition(&self, e: &i32, s: i32) -> Result<i32, NonEmpty<String>> { Ok(s + e) }
//! }
//!
//! # async fn example(pool: sqlx::PgPool) -> Result<(), Box<dyn std::error::Error>> {
//! let backend = Backend::builder(Counter, Counter.dsl::<i32, String>())
//!     .driver(SqlxDriver::for_namespace("counters", pool.clone()).await?)
//!     .build_default()
//!     .await?;
//!
//! let publisher = PulsarPublisher::builder("pulsar+ssl://broker.example.com:6651")
//!     .with_token("eyJ...")
//!     .with_namespace("my-tenant", "my-namespace")
//!     .with_fixed_topic("counters-notifications")
//!     .build()?;
//!
//! let relay = OutboxRelay::new(
//!     Arc::clone(backend.outbox()),
//!     Arc::new(publisher),
//!     MessageEncoder::<String>::serde(),
//!     RelayConfig::new("counters"),
//! )
//! .wake_on(backend.updates().outbox());
//!
//! relay
//!     .run_as_leader(LeaderLock::new(pool, "counters"), CancellationToken::new())
//!     .await?;
//! # Ok(()) }
//! ```
//!
//! ## Clever Cloud
//!
//! The Pulsar add-on injects `ADDON_PULSAR_BINARY_URL`, `ADDON_PULSAR_TOKEN`,
//! `ADDON_PULSAR_TENANT` and `ADDON_PULSAR_NAMESPACE`;
//! [`PulsarPublisherBuilder::from_clever_env`] reads them.
//!
//! ## Feature flags
//!
//! This crate has no Cargo feature flags of its own. The `pulsar` client is
//! built with TLS through `rustls` (no system OpenSSL), without OAuth2
//! authentication and without message compression; building it needs the
//! `protoc` protobuf compiler.

#![forbid(unsafe_code)]
#![warn(missing_docs)]
#![warn(rustdoc::broken_intra_doc_links, rustdoc::private_intra_doc_links)]
// `doc_auto_cfg` was merged into `doc_cfg` (Rust 1.92), which now shows
// feature-gated items on docs.rs automatically.
#![cfg_attr(docsrs, feature(doc_cfg))]

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use edomata_broker::{BrokerMessage, PublishError, Publisher};
use edomata_core::NonEmpty;
use pulsar::error::{ConnectionError, Error, ProducerError, ServiceDiscoveryError};
use pulsar::message::proto::ServerError;
use pulsar::producer::{Message, SendFuture};
use pulsar::routing_policy::RoutingPolicy;
use pulsar::{Authentication, Producer, ProducerOptions, Pulsar, TokioExecutor};
use tokio::sync::Mutex;

pub use pulsar;

type TopicFn = dyn Fn(&BrokerMessage) -> String + Send + Sync;

/// The tenant of short topic names unless
/// [`with_namespace`](PulsarPublisherBuilder::with_namespace) is called.
pub const DEFAULT_TENANT: &str = "public";

/// The namespace of short topic names unless
/// [`with_namespace`](PulsarPublisherBuilder::with_namespace) is called.
pub const DEFAULT_NAMESPACE: &str = "default";

/// The default maximum payload size: 5 MiB less the 10 KiB Pulsar reserves
/// for the frame and the metadata, i.e. Pulsar's documented default
/// `maxMessageSize` (5 MiB). See
/// [`with_max_message_size`](PulsarPublisherBuilder::with_max_message_size).
pub const DEFAULT_MAX_MESSAGE_SIZE: usize = 5 * 1024 * 1024 - 10 * 1024;

/// Why a [`PulsarPublisher`] could not be built.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[non_exhaustive]
pub enum BuildError {
    /// The service URL is not `pulsar://host:port` or
    /// `pulsar+ssl://host:port`.
    #[error(
        "invalid Pulsar service url {0:?}: expected pulsar://host:port or pulsar+ssl://host:port"
    )]
    InvalidServiceUrl(String),
    /// The tenant or the namespace is empty or contains a `/`.
    #[error("invalid tenant {tenant:?} or namespace {namespace:?}")]
    InvalidNamespace {
        /// The tenant as given.
        tenant: String,
        /// The namespace as given.
        namespace: String,
    },
    /// An environment variable required by
    /// [`PulsarPublisherBuilder::from_clever_env`] is missing or empty.
    #[error("missing environment variable {0}")]
    MissingEnv(&'static str),
}

/// A topic that cannot be resolved to a Pulsar topic name, see
/// [`resolve_topic`].
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("invalid Pulsar topic {topic:?}: {reason}")]
pub struct InvalidTopic {
    /// The topic as given.
    pub topic: String,
    /// What is wrong with it.
    pub reason: &'static str,
}

/// Publishes broker messages to Apache Pulsar.
///
/// Each [`BrokerMessage`] becomes one Pulsar message: the topic comes from
/// the topic function ([`default_topic`] unless set on the builder) and
/// [`resolve_topic`], the key is the stream id, the payload is the encoded
/// payload and every [`BrokerMessage::headers`] entry is a message property.
/// `publish` enqueues the whole batch, then waits for every send receipt; it
/// fails with the first failed receipt, after which the relay republishes the
/// batch (messages already acknowledged are delivered again: at-least-once).
///
/// Build it with [`PulsarPublisher::builder`]. Neither building it nor
/// creating the builder contacts the broker: the connection and the
/// producers are created at the first `publish`, so a wrong address shows up
/// as transient publish failures.
///
/// Concurrent `publish` calls are serialized while their messages are
/// enqueued (one mutex around the producers) and wait for their receipts
/// concurrently.
pub struct PulsarPublisher {
    config: ClientConfig,
    topic: Arc<TopicFn>,
    tenant: String,
    namespace: String,
    send_timeout: Duration,
    max_message_size: usize,
    state: Mutex<State>,
}

impl std::fmt::Debug for PulsarPublisher {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PulsarPublisher")
            .field("config", &self.config)
            .field("tenant", &self.tenant)
            .field("namespace", &self.namespace)
            .field("send_timeout", &self.send_timeout)
            .field("max_message_size", &self.max_message_size)
            .finish()
    }
}

/// How to reach the cluster. Never printed in full: the token stays out of
/// `Debug`.
#[derive(Clone)]
struct ClientConfig {
    service_url: String,
    authentication: Option<Authentication>,
    certificate_chain: Option<Vec<u8>>,
    allow_insecure_connection: Option<bool>,
    tls_hostname_verification: Option<bool>,
}

impl std::fmt::Debug for ClientConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ClientConfig")
            .field("service_url", &self.service_url)
            .field(
                "authentication",
                &self.authentication.as_ref().map(|a| &a.name),
            )
            .field("certificate_chain", &self.certificate_chain.is_some())
            .field("allow_insecure_connection", &self.allow_insecure_connection)
            .field("tls_hostname_verification", &self.tls_hostname_verification)
            .finish()
    }
}

#[derive(Default)]
struct State {
    client: Option<Pulsar<TokioExecutor>>,
    producers: HashMap<String, (u64, Producer<TokioExecutor>)>,
    next_id: u64,
}

impl State {
    /// Drops the producer of `topic` if it is still the instance `id` (a
    /// concurrent publish may already have replaced it).
    fn drop_producer(&mut self, topic: &str, id: u64) {
        if self.producers.get(topic).is_some_and(|(i, _)| *i == id) {
            self.producers.remove(topic);
        }
    }

    /// Drops the connection and every producer.
    fn reset(&mut self) {
        self.producers.clear();
        self.client = None;
    }

    /// The reaction to a failure of the producer `id` of `topic`.
    fn invalidate(&mut self, topic: &str, id: u64, error: &Error) {
        if resets_connection(error) {
            tracing::warn!(%error, "pulsar connection failed: dropping the client and its producers");
            self.reset();
        } else {
            tracing::warn!(%error, topic, "pulsar producer failed: dropping it");
            self.drop_producer(topic, id);
        }
    }
}

/// Builds a [`PulsarPublisher`].
///
/// ```
/// use std::time::Duration;
/// use edomata_broker::{BrokerMessage, MessageKind};
/// use edomata_pulsar::PulsarPublisher;
///
/// // Building does not connect: this runs without a broker.
/// let publisher = PulsarPublisher::builder("pulsar+ssl://localhost:6651")
///     .with_token("secret")
///     .with_namespace("acme", "prod")
///     .with_topic(|m: &BrokerMessage| format!("{}.{}", m.source, m.kind))
///     .with_send_timeout(Duration::from_secs(10))
///     .build()
///     .unwrap();
///
/// # let message = BrokerMessage {
/// #     id: BrokerMessage::outbox_id("accounts", 1), source: "accounts".into(),
/// #     kind: MessageKind::Notification, stream_id: "acc-1".into(), seq_nr: 1,
/// #     time: chrono::DateTime::UNIX_EPOCH, content_type: "application/json".into(),
/// #     payload: b"{}".to_vec(), correlation: None, causation: None,
/// #     extra_headers: Default::default(),
/// # };
/// assert_eq!(
///     publisher.topic_for(&message).unwrap(),
///     "persistent://acme/prod/accounts.notification"
/// );
/// // The token is not printed.
/// assert!(!format!("{publisher:?}").contains("secret"));
/// ```
pub struct PulsarPublisherBuilder {
    config: ClientConfig,
    topic: Arc<TopicFn>,
    tenant: String,
    namespace: String,
    send_timeout: Duration,
    max_message_size: usize,
}

impl std::fmt::Debug for PulsarPublisherBuilder {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PulsarPublisherBuilder")
            .field("config", &self.config)
            .field("tenant", &self.tenant)
            .field("namespace", &self.namespace)
            .field("send_timeout", &self.send_timeout)
            .field("max_message_size", &self.max_message_size)
            .finish()
    }
}

impl PulsarPublisher {
    /// A builder for the given service URL (`pulsar://host:port`, or
    /// `pulsar+ssl://host:port` for TLS) with the [`default_topic`], the
    /// `public/default` namespace, no authentication, a 30-second send
    /// timeout and [`DEFAULT_MAX_MESSAGE_SIZE`].
    pub fn builder(service_url: impl Into<String>) -> PulsarPublisherBuilder {
        PulsarPublisherBuilder {
            config: ClientConfig {
                service_url: service_url.into(),
                authentication: None,
                certificate_chain: None,
                allow_insecure_connection: None,
                tls_hostname_verification: None,
            },
            topic: Arc::new(default_topic),
            tenant: DEFAULT_TENANT.to_string(),
            namespace: DEFAULT_NAMESPACE.to_string(),
            send_timeout: Duration::from_secs(30),
            max_message_size: DEFAULT_MAX_MESSAGE_SIZE,
        }
    }

    /// The full topic name `message` goes to: the topic function's choice
    /// resolved by [`resolve_topic`] against the configured tenant and
    /// namespace.
    ///
    /// # Errors
    ///
    /// [`InvalidTopic`] when the function returned a malformed topic URL.
    pub fn topic_for(&self, message: &BrokerMessage) -> Result<String, InvalidTopic> {
        resolve_topic(&(self.topic)(message), &self.tenant, &self.namespace)
    }

    /// Closes the producers and drops the connection. Call it before
    /// shutting down; `publish` already waits for its own batch, so nothing
    /// is lost by not calling it. The publisher stays usable: the next
    /// `publish` reconnects.
    pub async fn close(&self) {
        let mut state = self.state.lock().await;
        let producers = std::mem::take(&mut state.producers);
        state.client = None;
        drop(state);
        for (topic, (_, mut producer)) in producers {
            if let Err(error) = producer.close().await {
                tracing::debug!(%error, topic, "closing a pulsar producer failed");
            }
        }
    }

    async fn connect(&self) -> Result<Pulsar<TokioExecutor>, Error> {
        let config = &self.config;
        let mut builder = Pulsar::builder(config.service_url.clone(), TokioExecutor);
        if let Some(authentication) = &config.authentication {
            builder = builder.with_auth(authentication.clone());
        }
        if let Some(chain) = &config.certificate_chain {
            builder = builder.with_certificate_chain(chain.clone());
        }
        if let Some(allow) = config.allow_insecure_connection {
            builder = builder.with_allow_insecure_connection(allow);
        }
        if let Some(enabled) = config.tls_hostname_verification {
            builder = builder.with_tls_hostname_verification_enabled(enabled);
        }
        builder.build().await
    }

    /// Enqueues `message` on the (cached or new) producer of `topic`; returns
    /// the producer instance and the receipt to await.
    async fn enqueue(
        &self,
        state: &mut State,
        topic: &str,
        message: Message,
    ) -> Result<(u64, SendFuture), (u64, Error)> {
        if !state.producers.contains_key(topic) {
            if state.client.is_none() {
                state.client = Some(self.connect().await.map_err(|e| (0, e))?);
            }
            let client = state.client.as_ref().expect("client was just created");
            let producer = client
                .producer()
                .with_topic(topic)
                .with_options(producer_options())
                .build()
                .await
                .map_err(|e| (0, e))?;
            state.next_id += 1;
            state
                .producers
                .insert(topic.to_string(), (state.next_id, producer));
        }
        let (id, producer) = state
            .producers
            .get_mut(topic)
            .expect("producer was just created");
        let id = *id;
        producer
            .send_non_blocking(message)
            .await
            .map(|receipt| (id, receipt))
            .map_err(|e| (id, e))
    }
}

/// The options of every producer.
///
/// - no batching: one message, one receipt, in send order;
/// - `block_queue_if_full`: a large batch waits for room in the producer's
///   queue instead of failing with `SlowDown`;
/// - round-robin *with a key* routes by a hash of the key, so that on a
///   partitioned topic a stream always lands in the same partition (without
///   the policy the client spreads messages round-robin and ignores the key).
fn producer_options() -> ProducerOptions {
    ProducerOptions {
        block_queue_if_full: true,
        routing_policy: Some(RoutingPolicy::RoundRobin),
        ..ProducerOptions::default()
    }
}

/// The default topic: the relay source with every character outside
/// `[A-Za-z0-9._-]` replaced by `-` (a safe subset of Pulsar's topic name
/// alphabet), that is one topic per relay source, see [`sanitize_topic`].
pub fn default_topic(message: &BrokerMessage) -> String {
    sanitize_topic(&message.source)
}

/// Makes a string a valid Pulsar topic name (the part after the namespace):
/// every character outside `[A-Za-z0-9._-]` becomes `-` (a safe subset of
/// the alphabet Pulsar accepts), and an empty name becomes `"edomata"`.
///
/// ```
/// use edomata_pulsar::sanitize_topic;
///
/// assert_eq!(sanitize_topic("accounts"), "accounts");
/// assert_eq!(sanitize_topic("billing/invoices:v1"), "billing-invoices-v1");
/// assert_eq!(sanitize_topic(""), "edomata");
/// ```
pub fn sanitize_topic(name: &str) -> String {
    let topic: String = name
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-') {
                c
            } else {
                '-'
            }
        })
        .collect();
    if topic.is_empty() {
        "edomata".to_string()
    } else {
        topic
    }
}

/// Resolves a topic to a full Pulsar topic name.
///
/// - a full URL (`persistent://tenant/namespace/topic`, or
///   `non-persistent://...`) is validated and returned as it is;
/// - a short name is sanitized ([`sanitize_topic`]) and placed in the given
///   tenant and namespace as a persistent topic.
///
/// ```
/// use edomata_pulsar::resolve_topic;
///
/// assert_eq!(
///     resolve_topic("accounts", "acme", "prod").unwrap(),
///     "persistent://acme/prod/accounts"
/// );
/// assert_eq!(
///     resolve_topic("persistent://other/ns/orders", "acme", "prod").unwrap(),
///     "persistent://other/ns/orders"
/// );
/// assert!(resolve_topic("persistent://missing-parts", "acme", "prod").is_err());
/// ```
///
/// # Errors
///
/// [`InvalidTopic`] when a URL has an unknown scheme or is not exactly
/// `scheme://tenant/namespace/topic` with three non-empty parts.
pub fn resolve_topic(topic: &str, tenant: &str, namespace: &str) -> Result<String, InvalidTopic> {
    let invalid = |reason| InvalidTopic {
        topic: topic.to_string(),
        reason,
    };
    match topic.split_once("://") {
        None => Ok(format!(
            "persistent://{tenant}/{namespace}/{}",
            sanitize_topic(topic)
        )),
        Some((scheme, rest)) => {
            if !matches!(scheme, "persistent" | "non-persistent") {
                return Err(invalid("the scheme must be persistent or non-persistent"));
            }
            let parts: Vec<&str> = rest.split('/').collect();
            if parts.len() != 3 || parts.iter().any(|p| p.is_empty()) {
                return Err(invalid("expected scheme://tenant/namespace/topic"));
            }
            Ok(topic.to_string())
        }
    }
}

impl PulsarPublisherBuilder {
    /// A builder configured from the environment variables the Pulsar
    /// add-on of Clever Cloud injects: `ADDON_PULSAR_BINARY_URL` (service
    /// URL), `ADDON_PULSAR_TOKEN`, `ADDON_PULSAR_TENANT` and
    /// `ADDON_PULSAR_NAMESPACE`. Further settings can be chained.
    ///
    /// # Errors
    ///
    /// [`BuildError::MissingEnv`] for the first variable that is unset or
    /// empty.
    pub fn from_clever_env() -> Result<Self, BuildError> {
        Self::from_env_with(|name| std::env::var(name).ok())
    }

    /// [`from_clever_env`](Self::from_clever_env) with a custom variable
    /// lookup.
    fn from_env_with(get: impl Fn(&str) -> Option<String>) -> Result<Self, BuildError> {
        let var = |name: &'static str| {
            get(name)
                .map(|v| v.trim().to_string())
                .filter(|v| !v.is_empty())
                .ok_or(BuildError::MissingEnv(name))
        };
        let url = var("ADDON_PULSAR_BINARY_URL")?;
        let token = var("ADDON_PULSAR_TOKEN")?;
        let tenant = var("ADDON_PULSAR_TENANT")?;
        let namespace = var("ADDON_PULSAR_NAMESPACE")?;
        Ok(PulsarPublisher::builder(url)
            .with_token(token)
            .with_namespace(tenant, namespace))
    }

    /// Authenticates with a JWT token. Use a `pulsar+ssl://` service URL: on
    /// `pulsar://` the token travels in clear text.
    pub fn with_token(self, token: impl Into<String>) -> Self {
        self.with_authentication(Authentication {
            name: "token".to_string(),
            data: token.into().into_bytes(),
        })
    }

    /// Authenticates with any method the client supports (the `name` is the
    /// Pulsar authentication method, `"token"` for JWT).
    pub fn with_authentication(mut self, authentication: Authentication) -> Self {
        self.config.authentication = Some(authentication);
        self
    }

    /// The tenant and namespace short topic names are placed in (default
    /// `public` / `default`). Full topic URLs ignore it.
    pub fn with_namespace(
        mut self,
        tenant: impl Into<String>,
        namespace: impl Into<String>,
    ) -> Self {
        self.tenant = tenant.into();
        self.namespace = namespace.into();
        self
    }

    /// Chooses the topic per message: a short name or a full topic URL (see
    /// [`resolve_topic`]). The function is called once per message; keep it
    /// deterministic so that a stream always goes to the same topic
    /// (per-stream ordering holds only within one topic), and keep its range
    /// small (one producer per topic, see the crate docs).
    pub fn with_topic<F>(mut self, topic: F) -> Self
    where
        F: Fn(&BrokerMessage) -> String + Send + Sync + 'static,
    {
        self.topic = Arc::new(topic);
        self
    }

    /// Sends every message to one topic (a short name or a full topic URL);
    /// streams stay ordered through the message key.
    pub fn with_fixed_topic(self, topic: impl Into<String>) -> Self {
        let topic = topic.into();
        self.with_topic(move |_| topic.clone())
    }

    /// How long to wait for the connection, the producer creation, the
    /// enqueueing of a batch and then its receipts (each phase has this
    /// budget). A timeout is a transient failure.
    pub fn with_send_timeout(mut self, timeout: Duration) -> Self {
        self.send_timeout = timeout;
        self
    }

    /// The largest payload accepted, in bytes (default
    /// [`DEFAULT_MAX_MESSAGE_SIZE`]). A larger message fails the batch with
    /// [`PublishError::Permanent`] before anything is sent, instead of being
    /// retried forever. Raise it only together with the broker's
    /// `maxMessageSize`.
    pub fn with_max_message_size(mut self, bytes: usize) -> Self {
        self.max_message_size = bytes;
        self
    }

    /// A PEM certificate chain trusted to authenticate the broker over
    /// `pulsar+ssl://`, for private authorities.
    pub fn with_certificate_chain(mut self, pem: Vec<u8>) -> Self {
        self.config.certificate_chain = Some(pem);
        self
    }

    /// Accepts any server certificate over `pulsar+ssl://`. Testing only.
    pub fn with_allow_insecure_connection(mut self, allow: bool) -> Self {
        self.config.allow_insecure_connection = Some(allow);
        self
    }

    /// Enables or disables the verification of the broker's host name over
    /// `pulsar+ssl://`.
    pub fn with_tls_hostname_verification_enabled(mut self, enabled: bool) -> Self {
        self.config.tls_hostname_verification = Some(enabled);
        self
    }

    /// Creates the publisher. It does not contact the broker.
    ///
    /// # Errors
    ///
    /// [`BuildError`] when the service URL does not start with `pulsar://`
    /// or `pulsar+ssl://`, or the tenant or namespace is empty or contains a
    /// `/`.
    pub fn build(self) -> Result<PulsarPublisher, BuildError> {
        let url = &self.config.service_url;
        let secure = url.starts_with("pulsar+ssl://");
        let host = url
            .strip_prefix("pulsar+ssl://")
            .or_else(|| url.strip_prefix("pulsar://"))
            .unwrap_or_default();
        if host.is_empty() {
            return Err(BuildError::InvalidServiceUrl(url.clone()));
        }
        let bad = |s: &str| s.is_empty() || s.contains('/');
        if bad(&self.tenant) || bad(&self.namespace) {
            return Err(BuildError::InvalidNamespace {
                tenant: self.tenant,
                namespace: self.namespace,
            });
        }
        if !secure && self.config.authentication.is_some() {
            tracing::warn!(
                "pulsar authentication over a plain pulsar:// url: credentials are not encrypted"
            );
        }
        Ok(PulsarPublisher {
            config: self.config,
            topic: self.topic,
            tenant: self.tenant,
            namespace: self.namespace,
            send_timeout: self.send_timeout,
            max_message_size: self.max_message_size,
            state: Mutex::new(State::default()),
        })
    }
}

/// The message properties of a broker message: the id and every metadata
/// header of [`BrokerMessage::headers`].
fn properties_of(message: &BrokerMessage) -> HashMap<String, String> {
    message.headers().into_iter().collect()
}

/// The Pulsar message for a broker message: payload, stream id as key,
/// headers as properties, the message time as event time.
fn pulsar_message(message: &BrokerMessage) -> Message {
    Message {
        payload: message.payload.clone(),
        properties: properties_of(message),
        partition_key: Some(message.stream_id.clone()),
        event_time: u64::try_from(message.time.timestamp_millis()).ok(),
        ..Message::default()
    }
}

/// The Pulsar server error behind `error`, when there is one.
fn server_error(error: &Error) -> Option<ServerError> {
    match error {
        Error::Connection(ConnectionError::PulsarError(kind, _))
        | Error::Producer(ProducerError::Connection(ConnectionError::PulsarError(kind, _)))
        | Error::ServiceDiscovery(ServiceDiscoveryError::Query(kind, _)) => *kind,
        Error::Producer(ProducerError::Batch(inner)) => server_error(inner),
        _ => None,
    }
}

/// Whether a failure says that retrying the same batch can never succeed.
fn is_permanent(error: &Error) -> bool {
    matches!(
        server_error(error),
        Some(
            ServerError::InvalidTopicName
                | ServerError::AuthenticationError
                | ServerError::AuthorizationError
                | ServerError::NotAllowedError
                | ServerError::IncompatibleSchema
                | ServerError::UnsupportedVersionError
                | ServerError::TopicTerminatedError
        )
    )
}

/// Whether a failure is about the connection rather than one producer: the
/// client is rebuilt too. A server error answers on a healthy connection.
fn resets_connection(error: &Error) -> bool {
    match error {
        Error::Connection(ConnectionError::PulsarError(..))
        | Error::Producer(ProducerError::Connection(ConnectionError::PulsarError(..)))
        | Error::ServiceDiscovery(ServiceDiscoveryError::Query(..)) => false,
        Error::Connection(_)
        | Error::Producer(
            ProducerError::Connection(_) | ProducerError::Io(_) | ProducerError::Closed,
        )
        | Error::ServiceDiscovery(_) => true,
        Error::Producer(ProducerError::Batch(inner)) => resets_connection(inner),
        _ => false,
    }
}

fn classify(error: Error) -> PublishError {
    if is_permanent(&error) {
        PublishError::Permanent(Box::new(error))
    } else {
        PublishError::Transient(Box::new(error))
    }
}

#[async_trait]
impl Publisher for PulsarPublisher {
    async fn publish(&self, batch: &NonEmpty<BrokerMessage>) -> Result<(), PublishError> {
        // Validate everything before any I/O: a message that can never be
        // sent must not be retried, and nothing is half-published for it.
        let mut prepared = Vec::with_capacity(batch.len());
        for message in batch.iter() {
            if message.payload.len() > self.max_message_size {
                return Err(PublishError::Permanent(
                    format!(
                        "message {} has a {}-byte payload, above the {}-byte maximum",
                        message.id,
                        message.payload.len(),
                        self.max_message_size
                    )
                    .into(),
                ));
            }
            let topic = self
                .topic_for(message)
                .map_err(|e| PublishError::Permanent(Box::new(e)))?;
            prepared.push((topic, pulsar_message(message)));
        }

        // Enqueue in batch order (per-producer order is the send order),
        // under the lock, then wait for the receipts without it.
        let mut pending: Vec<(String, u64, SendFuture)> = Vec::with_capacity(prepared.len());
        let enqueued = {
            let mut state = self.state.lock().await;
            let state = &mut *state;
            let enqueue_all = async {
                for (topic, message) in prepared {
                    match self.enqueue(state, &topic, message).await {
                        Ok((id, receipt)) => pending.push((topic, id, receipt)),
                        Err((id, error)) => {
                            state.invalidate(&topic, id, &error);
                            return Err(classify(error));
                        }
                    }
                }
                Ok(())
            };
            match tokio::time::timeout(self.send_timeout, enqueue_all).await {
                Ok(result) => result,
                Err(_) => {
                    state.reset();
                    Err(PublishError::transient(format!(
                        "timed out after {:?} connecting to Pulsar or enqueueing the batch",
                        self.send_timeout
                    )))
                }
            }
        };
        enqueued?;

        let producers: Vec<(String, u64)> = pending
            .iter()
            .map(|(topic, id, _)| (topic.clone(), *id))
            .collect();
        let acknowledged = async {
            for (topic, id, receipt) in pending {
                let receipt = receipt.await.map_err(|e| (topic.clone(), id, e))?;
                tracing::trace!(topic, ?receipt.message_id, "pulsar delivery acknowledged");
            }
            Ok::<(), (String, u64, Error)>(())
        };
        match tokio::time::timeout(self.send_timeout, acknowledged).await {
            Ok(Ok(())) => Ok(()),
            Ok(Err((topic, id, error))) => {
                self.state.lock().await.invalidate(&topic, id, &error);
                Err(classify(error))
            }
            Err(_) => {
                // Unknown which producer is stuck: drop every one of the batch.
                let mut state = self.state.lock().await;
                for (topic, id) in &producers {
                    state.drop_producer(topic, *id);
                }
                Err(PublishError::transient(format!(
                    "timed out after {:?} waiting for the broker to acknowledge the batch",
                    self.send_timeout
                )))
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use edomata_broker::{MessageKind, headers};

    fn message(source: &str, stream: &str) -> BrokerMessage {
        BrokerMessage {
            id: BrokerMessage::outbox_id(source, 7),
            source: source.into(),
            kind: MessageKind::Notification,
            stream_id: stream.into(),
            seq_nr: 7,
            time: chrono::DateTime::from_timestamp(1_700_000_000, 0).unwrap(),
            content_type: "application/json".into(),
            payload: br#"{"n":1}"#.to_vec(),
            correlation: Some("corr-1".into()),
            causation: None,
            extra_headers: [("x-tenant".to_string(), "acme".to_string())]
                .into_iter()
                .collect(),
        }
    }

    fn server(kind: ServerError) -> Error {
        Error::Connection(ConnectionError::PulsarError(
            Some(kind),
            Some("boom".into()),
        ))
    }

    // ---- topic resolution and sanitizing ---------------------------------

    #[test]
    fn short_names_are_placed_in_the_tenant_and_namespace() {
        assert_eq!(
            resolve_topic("accounts", "acme", "prod").unwrap(),
            "persistent://acme/prod/accounts"
        );
    }

    #[test]
    fn short_names_are_sanitized() {
        assert_eq!(
            resolve_topic("billing/invoices:v1", "t", "n").unwrap(),
            "persistent://t/n/billing-invoices-v1"
        );
        assert_eq!(
            resolve_topic("", "t", "n").unwrap(),
            "persistent://t/n/edomata"
        );
        assert_eq!(sanitize_topic("a.b_c-d"), "a.b_c-d");
        assert_eq!(sanitize_topic("é à"), "---");
        assert_eq!(sanitize_topic(""), "edomata");
    }

    #[test]
    fn full_urls_are_kept_verbatim_and_ignore_the_namespace() {
        for url in [
            "persistent://other/ns/orders",
            "non-persistent://other/ns/orders",
            "persistent://other/ns/orders.v1",
        ] {
            assert_eq!(resolve_topic(url, "acme", "prod").unwrap(), url);
        }
    }

    #[test]
    fn malformed_urls_are_rejected() {
        for url in [
            "persistent://orders",
            "persistent://ns/orders",
            "persistent://a/b/c/d",
            "persistent://a//c",
            "persistent:///b/c",
            "kafka://a/b/c",
            "://a/b/c",
        ] {
            assert!(
                resolve_topic(url, "t", "n").is_err(),
                "{url} must be rejected"
            );
        }
    }

    #[test]
    fn default_topic_is_the_sanitized_source() {
        assert_eq!(default_topic(&message("accounts", "s")), "accounts");
        assert_eq!(default_topic(&message("\"billing\".x", "s")), "-billing-.x");
    }

    // ---- message mapping --------------------------------------------------

    #[test]
    fn key_is_the_stream_id_and_properties_are_the_headers() {
        let m = message("accounts", "acc-42");
        let out = pulsar_message(&m);
        assert_eq!(out.partition_key.as_deref(), Some("acc-42"));
        assert_eq!(out.payload, br#"{"n":1}"#);
        assert_eq!(out.event_time, Some(1_700_000_000_000));
        assert_eq!(out.properties[headers::ID], "accounts:outbox:7");
        assert_eq!(out.properties[headers::SOURCE], "accounts");
        assert_eq!(out.properties[headers::KIND], "notification");
        assert_eq!(out.properties[headers::STREAM], "acc-42");
        assert_eq!(out.properties[headers::SEQ_NR], "7");
        assert_eq!(out.properties[headers::CONTENT_TYPE], "application/json");
        assert_eq!(out.properties[headers::CORRELATION], "corr-1");
        assert_eq!(out.properties["x-tenant"], "acme");
        assert!(out.properties.contains_key(headers::TIME));
        assert!(!out.properties.contains_key(headers::CAUSATION));
        assert_eq!(out.properties.len(), m.headers().len());
    }

    #[test]
    fn pre_epoch_times_have_no_event_time() {
        let mut m = message("a", "s");
        m.time = chrono::DateTime::from_timestamp(-5, 0).unwrap();
        assert_eq!(pulsar_message(&m).event_time, None);
    }

    #[test]
    fn producers_route_by_key_and_do_not_batch() {
        let options = producer_options();
        assert!(matches!(
            options.routing_policy,
            Some(RoutingPolicy::RoundRobin)
        ));
        assert!(options.block_queue_if_full);
        assert_eq!(options.batch_size, None);
    }

    // ---- error classification ---------------------------------------------

    #[test]
    fn rejections_are_permanent() {
        for kind in [
            ServerError::InvalidTopicName,
            ServerError::AuthenticationError,
            ServerError::AuthorizationError,
            ServerError::NotAllowedError,
            ServerError::IncompatibleSchema,
            ServerError::UnsupportedVersionError,
            ServerError::TopicTerminatedError,
        ] {
            assert!(
                matches!(classify(server(kind)), PublishError::Permanent(_)),
                "{kind:?} must be permanent"
            );
        }
    }

    #[test]
    fn permanent_server_errors_are_found_in_every_wrapper() {
        let kind = || Some(ServerError::AuthorizationError);
        let pulsar_error = || ConnectionError::PulsarError(kind(), None);
        for error in [
            Error::Producer(ProducerError::Connection(pulsar_error())),
            Error::ServiceDiscovery(ServiceDiscoveryError::Query(kind(), None)),
            Error::Producer(ProducerError::Batch(Arc::new(Error::Connection(
                pulsar_error(),
            )))),
        ] {
            assert!(matches!(classify(error), PublishError::Permanent(_)));
        }
    }

    #[test]
    fn connection_problems_and_unknowns_are_transient() {
        for kind in [
            ServerError::ServiceNotReady,
            ServerError::TooManyRequests,
            ServerError::ProducerBusy,
            ServerError::ProducerBlockedQuotaExceededError,
            ServerError::PersistenceError,
            ServerError::MetadataError,
            ServerError::UnknownError,
            ServerError::TopicNotFound,
            ServerError::ProducerFenced,
        ] {
            assert!(
                matches!(classify(server(kind)), PublishError::Transient(_)),
                "{kind:?} must be transient"
            );
        }
        for error in [
            Error::Connection(ConnectionError::Disconnected),
            Error::Connection(ConnectionError::SlowDown),
            Error::Connection(ConnectionError::PulsarError(None, None)),
            Error::Producer(ProducerError::Closed),
            Error::Producer(ProducerError::Fenced),
            Error::Producer(ProducerError::Custom(
                "producer unexpectedly disconnected".into(),
            )),
            Error::Custom("who knows".into()),
            Error::Executor,
        ] {
            assert!(
                matches!(classify(error), PublishError::Transient(_)),
                "unknown errors are transient"
            );
        }
    }

    #[test]
    fn only_connection_level_failures_rebuild_the_client() {
        assert!(resets_connection(&Error::Connection(
            ConnectionError::Disconnected
        )));
        assert!(resets_connection(&Error::Producer(ProducerError::Closed)));
        assert!(resets_connection(&Error::Producer(ProducerError::Batch(
            Arc::new(Error::Connection(ConnectionError::Disconnected))
        ))));
        // A server error came over a healthy connection: only the producer goes.
        assert!(!resets_connection(&server(ServerError::AuthorizationError)));
        assert!(!resets_connection(&server(ServerError::ServiceNotReady)));
        assert!(!resets_connection(&Error::Producer(ProducerError::Fenced)));
        assert!(!resets_connection(&Error::Custom("x".into())));
    }

    // ---- builder ----------------------------------------------------------

    #[test]
    fn builder_defaults() {
        let p = PulsarPublisher::builder("pulsar://localhost:6650")
            .build()
            .unwrap();
        assert_eq!(
            p.topic_for(&message("accounts", "s")).unwrap(),
            "persistent://public/default/accounts"
        );
        assert_eq!(p.send_timeout, Duration::from_secs(30));
        assert_eq!(p.max_message_size, DEFAULT_MAX_MESSAGE_SIZE);
        assert_eq!(DEFAULT_MAX_MESSAGE_SIZE, 5_232_640);
    }

    #[test]
    fn builder_options() {
        let p = PulsarPublisher::builder("pulsar+ssl://b:6651")
            .with_namespace("acme", "prod")
            .with_fixed_topic("all")
            .with_send_timeout(Duration::from_secs(3))
            .with_max_message_size(10)
            .build()
            .unwrap();
        assert_eq!(
            p.topic_for(&message("x", "s")).unwrap(),
            "persistent://acme/prod/all"
        );
        assert_eq!(p.send_timeout, Duration::from_secs(3));
        assert_eq!(p.max_message_size, 10);

        let p = PulsarPublisher::builder("pulsar://b:6650")
            .with_fixed_topic("persistent://t/n/full")
            .build()
            .unwrap();
        assert_eq!(
            p.topic_for(&message("x", "s")).unwrap(),
            "persistent://t/n/full"
        );

        let p = PulsarPublisher::builder("pulsar://b:6650")
            .with_topic(|m| format!("{}-{}", m.source, m.kind))
            .build()
            .unwrap();
        assert_eq!(
            p.topic_for(&message("acc", "s")).unwrap(),
            "persistent://public/default/acc-notification"
        );
    }

    #[test]
    fn builder_rejects_bad_urls_and_namespaces() {
        for url in [
            "",
            "localhost:6650",
            "http://x",
            "pulsar://",
            "pulsar+ssl://",
        ] {
            assert_eq!(
                PulsarPublisher::builder(url).build().unwrap_err(),
                BuildError::InvalidServiceUrl(url.to_string())
            );
        }
        for (tenant, namespace) in [("", "n"), ("t", ""), ("a/b", "n"), ("t", "a/b")] {
            assert!(matches!(
                PulsarPublisher::builder("pulsar://b:6650")
                    .with_namespace(tenant, namespace)
                    .build(),
                Err(BuildError::InvalidNamespace { .. })
            ));
        }
    }

    #[test]
    fn debug_output_hides_the_token() {
        let builder = PulsarPublisher::builder("pulsar+ssl://b:6651").with_token("s3cr3t-token");
        assert!(!format!("{builder:?}").contains("s3cr3t"));
        assert!(!format!("{:?}", builder.build().unwrap()).contains("s3cr3t"));
    }

    #[test]
    fn clever_env() {
        let env: HashMap<&str, &str> = [
            ("ADDON_PULSAR_BINARY_URL", "pulsar+ssl://clever:6651"),
            ("ADDON_PULSAR_TOKEN", " tok "),
            ("ADDON_PULSAR_TENANT", "orga_1"),
            ("ADDON_PULSAR_NAMESPACE", "addon_2"),
        ]
        .into_iter()
        .collect();
        let lookup = |env: HashMap<&'static str, &'static str>| {
            move |name: &str| env.get(name).map(|v| v.to_string())
        };
        let builder = PulsarPublisherBuilder::from_env_with(lookup(env.clone())).unwrap();
        assert_eq!(builder.config.service_url, "pulsar+ssl://clever:6651");
        assert_eq!(builder.config.authentication.as_ref().unwrap().data, b"tok");
        let p = builder.build().unwrap();
        assert_eq!(
            p.topic_for(&message("accounts", "s")).unwrap(),
            "persistent://orga_1/addon_2/accounts"
        );

        for missing in [
            "ADDON_PULSAR_BINARY_URL",
            "ADDON_PULSAR_TOKEN",
            "ADDON_PULSAR_TENANT",
            "ADDON_PULSAR_NAMESPACE",
        ] {
            let mut env = env.clone();
            env.remove(missing);
            assert_eq!(
                PulsarPublisherBuilder::from_env_with(lookup(env)).unwrap_err(),
                BuildError::MissingEnv(missing)
            );
        }
        let mut blank = env.clone();
        blank.insert("ADDON_PULSAR_TOKEN", "  ");
        assert_eq!(
            PulsarPublisherBuilder::from_env_with(lookup(blank)).unwrap_err(),
            BuildError::MissingEnv("ADDON_PULSAR_TOKEN")
        );
    }

    // ---- publish: failures before any connection --------------------------

    fn batch(messages: Vec<BrokerMessage>) -> NonEmpty<BrokerMessage> {
        NonEmpty::from_vec(messages).unwrap()
    }

    #[tokio::test]
    async fn oversized_messages_are_permanent_without_connecting() {
        let p = PulsarPublisher::builder("pulsar://127.0.0.1:1")
            .with_max_message_size(3)
            .build()
            .unwrap();
        let err = p
            .publish(&batch(vec![message("a", "s")]))
            .await
            .unwrap_err();
        assert!(matches!(err, PublishError::Permanent(_)), "{err}");
    }

    #[tokio::test]
    async fn invalid_topic_urls_are_permanent_without_connecting() {
        let p = PulsarPublisher::builder("pulsar://127.0.0.1:1")
            .with_fixed_topic("persistent://oops")
            .build()
            .unwrap();
        let err = p
            .publish(&batch(vec![message("a", "s")]))
            .await
            .unwrap_err();
        assert!(matches!(err, PublishError::Permanent(_)), "{err}");
    }

    #[tokio::test]
    async fn an_unreachable_broker_is_transient() {
        let p = PulsarPublisher::builder("pulsar://127.0.0.1:1")
            .with_send_timeout(Duration::from_secs(5))
            .build()
            .unwrap();
        let err = p
            .publish(&batch(vec![message("a", "s")]))
            .await
            .unwrap_err();
        assert!(matches!(err, PublishError::Transient(_)), "{err}");
        // Nothing stays cached: the next publish starts from scratch.
        let state = p.state.lock().await;
        assert!(state.client.is_none() && state.producers.is_empty());
    }
}
