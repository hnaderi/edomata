//! The broker-agnostic message.

use std::collections::BTreeMap;
use std::fmt;

use chrono::{DateTime, Utc};

/// Names of the standard headers every publisher attaches to a message.
///
/// [`BrokerMessage::headers`](super::BrokerMessage::headers) returns them
/// with their values. `edomata-kafka` sends all of them as Kafka headers;
/// `edomata-rabbitmq` maps [`ID`](headers::ID) to the AMQP `message_id`
/// property, [`CONTENT_TYPE`](headers::CONTENT_TYPE) to `content_type`, and
/// sends the others as AMQP headers.
pub mod headers {
    /// The stable message id ([`super::BrokerMessage::id`]).
    pub const ID: &str = "edomata-id";
    /// The relay source (aggregate namespace).
    pub const SOURCE: &str = "edomata-source";
    /// `"notification"` or `"event"`.
    pub const KIND: &str = "edomata-kind";
    /// The stream (aggregate) id.
    pub const STREAM: &str = "edomata-stream";
    /// The outbox or journal sequence number.
    pub const SEQ_NR: &str = "edomata-seqnr";
    /// The time the item was written, RFC 3339.
    pub const TIME: &str = "edomata-time";
    /// The payload content type.
    pub const CONTENT_TYPE: &str = "content-type";
    /// The correlation id of the originating command chain.
    pub const CORRELATION: &str = "correlation-id";
    /// The causation id (the command that produced the item).
    pub const CAUSATION: &str = "causation-id";
    /// The journal event id (journal messages only).
    pub const EVENT_ID: &str = "edomata-event-id";
    /// The aggregate version after the event (journal messages only).
    pub const VERSION: &str = "edomata-version";
}

/// What a message carries: which relay produced it.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum MessageKind {
    /// An outbox notification.
    Notification,
    /// A journaled event.
    Event,
}

impl MessageKind {
    /// The value of the [`headers::KIND`] header: `"notification"` or
    /// `"event"`.
    pub fn as_str(self) -> &'static str {
        match self {
            MessageKind::Notification => "notification",
            MessageKind::Event => "event",
        }
    }
}

impl fmt::Display for MessageKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// A message handed to a [`Publisher`](crate::Publisher).
///
/// Delivery is at-least-once, so every message has a stable, deterministic
/// [`id`](Self::id) derived from the relay source and the outbox or journal
/// sequence number ([`BrokerMessage::outbox_id`], [`BrokerMessage::journal_id`]);
/// consumers deduplicate on it. Per-stream ordering is preserved by
/// publishing in sequence-number order and keying / routing by
/// [`stream_id`](Self::stream_id).
///
/// Relays build messages with
/// [`OutboxRelay::message`](crate::OutboxRelay::message) and
/// [`JournalRelay::message`](crate::JournalRelay::message); publishers read
/// them.
///
/// ```
/// use edomata_broker::{BrokerMessage, MessageKind, headers};
///
/// let message = BrokerMessage {
///     id: BrokerMessage::outbox_id("accounts", 42),
///     source: "accounts".into(),
///     kind: MessageKind::Notification,
///     stream_id: "acc-1".into(),
///     seq_nr: 42,
///     time: chrono::DateTime::UNIX_EPOCH,
///     content_type: "application/json".into(),
///     payload: br#"{"balance":10}"#.to_vec(),
///     correlation: Some("corr-1".into()),
///     causation: None,
///     extra_headers: Default::default(),
/// };
/// assert_eq!(message.id, "accounts:outbox:42");
/// assert_eq!(message.payload_text(), r#"{"balance":10}"#);
///
/// let hs = message.headers();
/// assert_eq!(hs[0], (headers::ID.to_string(), "accounts:outbox:42".to_string()));
/// assert!(hs.contains(&(headers::KIND.to_string(), "notification".to_string())));
/// assert!(hs.contains(&(headers::CORRELATION.to_string(), "corr-1".to_string())));
/// assert!(!hs.iter().any(|(k, _)| k == headers::CAUSATION)); // `None` is omitted
/// ```
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BrokerMessage {
    /// Stable, deterministic id: [`BrokerMessage::outbox_id`] or
    /// [`BrokerMessage::journal_id`]. Republishing an item yields the same
    /// id, so consumers deduplicate on it.
    pub id: String,
    /// The relay source (typically the aggregate namespace).
    pub source: String,
    /// Notification or event.
    pub kind: MessageKind,
    /// The stream (aggregate) the message belongs to.
    pub stream_id: String,
    /// The outbox or journal sequence number.
    pub seq_nr: i64,
    /// When the item was written.
    pub time: DateTime<Utc>,
    /// The payload content type (`application/json` for serde payloads).
    pub content_type: String,
    /// The encoded payload.
    pub payload: Vec<u8>,
    /// Correlation id of the originating command chain.
    pub correlation: Option<String>,
    /// Causation id (the command that produced the item).
    pub causation: Option<String>,
    /// Additional headers (journal messages carry the event id and version).
    pub extra_headers: BTreeMap<String, String>,
}

impl BrokerMessage {
    /// The id of an outbox item: `"{source}:outbox:{seq_nr}"`. Outbox
    /// sequence numbers are unique per outbox table, so the id is unique per
    /// source.
    pub fn outbox_id(source: &str, seq_nr: i64) -> String {
        format!("{source}:outbox:{seq_nr}")
    }

    /// The id of a journal event: `"{source}:journal:{seq_nr}"`, unique per
    /// source like [`outbox_id`](Self::outbox_id).
    pub fn journal_id(source: &str, seq_nr: i64) -> String {
        format!("{source}:journal:{seq_nr}")
    }

    /// Every header as `(name, value)` pairs, in a stable order: the
    /// standard [`headers`] `ID`, `SOURCE`, `KIND`, `STREAM`, `SEQ_NR`, `TIME`
    /// and `CONTENT_TYPE`, then `CORRELATION` and `CAUSATION` when present,
    /// then [`extra_headers`](Self::extra_headers) sorted by name.
    pub fn headers(&self) -> Vec<(String, String)> {
        let mut out = vec![
            (headers::ID.to_string(), self.id.clone()),
            (headers::SOURCE.to_string(), self.source.clone()),
            (headers::KIND.to_string(), self.kind.as_str().to_string()),
            (headers::STREAM.to_string(), self.stream_id.clone()),
            (headers::SEQ_NR.to_string(), self.seq_nr.to_string()),
            (headers::TIME.to_string(), self.time.to_rfc3339()),
            (headers::CONTENT_TYPE.to_string(), self.content_type.clone()),
        ];
        if let Some(c) = &self.correlation {
            out.push((headers::CORRELATION.to_string(), c.clone()));
        }
        if let Some(c) = &self.causation {
            out.push((headers::CAUSATION.to_string(), c.clone()));
        }
        out.extend(
            self.extra_headers
                .iter()
                .map(|(k, v)| (k.clone(), v.clone())),
        );
        out
    }

    /// The payload as UTF-8 text, for logs and tests; invalid UTF-8 is
    /// replaced with `U+FFFD`.
    pub fn payload_text(&self) -> String {
        String::from_utf8_lossy(&self.payload).into_owned()
    }
}
