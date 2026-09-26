//! Streaming the journal (raw events) to a broker from a checkpoint.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use edomata_backend::eventsourcing::JournalReader;
use edomata_backend::{EventMessage, Payload};
use edomata_core::NonEmpty;
use futures::stream::BoxStream;
use futures::{StreamExt, TryStreamExt};
use tokio_util::sync::CancellationToken;

use crate::message::headers;
use crate::postgres::LeaderLock;
use crate::runner::{publish_with_retry, run_loop};
use crate::{
    BrokerMessage, MessageEncoder, MessageKind, Publisher, RelayConfig, RelayError, RelayMetrics,
};

/// Where a [`JournalRelay`] remembers the last published sequence number.
#[async_trait]
pub trait CheckpointStore: Send + Sync {
    /// The last published journal sequence number of `relay`, if any.
    async fn load(&self, relay: &str) -> Result<Option<i64>, RelayError>;

    /// Records the last published journal sequence number of `relay`.
    async fn save(&self, relay: &str, seq_nr: i64) -> Result<(), RelayError>;
}

/// An in-memory [`CheckpointStore`] (tests, single-process tools).
#[derive(Debug, Default)]
pub struct InMemoryCheckpointStore {
    checkpoints: Mutex<HashMap<String, i64>>,
}

impl InMemoryCheckpointStore {
    /// An empty store.
    pub fn new() -> Self {
        Self::default()
    }

    /// The stored checkpoint of `relay`.
    pub fn get(&self, relay: &str) -> Option<i64> {
        self.checkpoints.lock().unwrap().get(relay).copied()
    }
}

#[async_trait]
impl CheckpointStore for InMemoryCheckpointStore {
    async fn load(&self, relay: &str) -> Result<Option<i64>, RelayError> {
        Ok(self.get(relay))
    }

    async fn save(&self, relay: &str, seq_nr: i64) -> Result<(), RelayError> {
        self.checkpoints
            .lock()
            .unwrap()
            .insert(relay.to_string(), seq_nr);
        Ok(())
    }
}

/// Tails the journal and publishes every event (not just notifications)
/// after the checkpoint stored in a [`CheckpointStore`]; the checkpoint
/// advances only after the broker acknowledged a batch.
///
/// Same loop, ordering, batching, retry and leader election as
/// [`OutboxRelay`](crate::OutboxRelay); wake-ups typically come from the
/// backend's journal signal or a PostgreSQL `LISTEN` stream.
pub struct JournalRelay<E> {
    reader: Arc<dyn JournalReader<E>>,
    checkpoints: Arc<dyn CheckpointStore>,
    publisher: Arc<dyn Publisher>,
    encoder: MessageEncoder<E>,
    config: RelayConfig,
    name: String,
    metrics: Arc<RelayMetrics>,
    wakeups: Mutex<Vec<BoxStream<'static, ()>>>,
}

impl<E> std::fmt::Debug for JournalRelay<E> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("JournalRelay")
            .field("name", &self.name)
            .field("config", &self.config)
            .finish()
    }
}

impl<E: Payload> JournalRelay<E> {
    /// A relay from a journal reader to a publisher, checkpointed under the
    /// name `"{source}:journal"`.
    pub fn new(
        reader: Arc<dyn JournalReader<E>>,
        checkpoints: Arc<dyn CheckpointStore>,
        publisher: Arc<dyn Publisher>,
        encoder: MessageEncoder<E>,
        config: RelayConfig,
    ) -> Self {
        let name = format!("{}:journal", config.source);
        Self {
            reader,
            checkpoints,
            publisher,
            encoder,
            config,
            name,
            metrics: Arc::new(RelayMetrics::new()),
            wakeups: Mutex::new(Vec::new()),
        }
    }

    /// Uses another checkpoint name (to run several independent relays on
    /// one journal).
    pub fn with_name(mut self, name: impl Into<String>) -> Self {
        self.name = name.into();
        self
    }

    /// Adds a wake-up stream: every element triggers a pass.
    pub fn wake_on(self, wakeups: BoxStream<'static, ()>) -> Self {
        self.wakeups.lock().unwrap().push(wakeups);
        self
    }

    /// The relay's counters.
    pub fn metrics(&self) -> Arc<RelayMetrics> {
        Arc::clone(&self.metrics)
    }

    /// The checkpoint name.
    pub fn name(&self) -> &str {
        &self.name
    }

    /// The message published for a journaled event; its id is
    /// [`BrokerMessage::journal_id`] of the relay source and the event's
    /// sequence number, and the event id and version travel as headers.
    pub fn message(&self, event: &EventMessage<E>) -> Result<BrokerMessage, RelayError> {
        let mut extra_headers = std::collections::BTreeMap::new();
        extra_headers.insert(headers::EVENT_ID.to_string(), event.metadata.id.to_string());
        extra_headers.insert(
            headers::VERSION.to_string(),
            event.metadata.version.to_string(),
        );
        Ok(BrokerMessage {
            id: BrokerMessage::journal_id(&self.config.source, event.metadata.seq_nr),
            source: self.config.source.clone(),
            kind: MessageKind::Event,
            stream_id: event.metadata.stream.clone(),
            seq_nr: event.metadata.seq_nr,
            time: event.metadata.time,
            content_type: self.encoder.content_type().to_string(),
            payload: self.encoder.encode(&event.payload)?,
            correlation: None,
            causation: None,
            extra_headers,
        })
    }

    /// One pass: publishes every event after the checkpoint and advances it.
    /// Returns the number of events published.
    pub async fn relay_once(&self) -> Result<usize, RelayError> {
        self.pass(&CancellationToken::new()).await
    }

    async fn pass(&self, cancel: &CancellationToken) -> Result<usize, RelayError> {
        let from = self.checkpoints.load(&self.name).await?.unwrap_or(0);
        let mut batches = self
            .reader
            .read_all_after(from)
            .try_chunks(self.config.batch_size);
        let mut total = 0usize;
        while let Some(batch) = batches.next().await {
            let batch = batch.map_err(|e| e.1)?;
            let Some(events) = NonEmpty::from_vec(batch) else {
                continue;
            };
            let messages = events
                .iter()
                .map(|e| self.message(e))
                .collect::<Result<Vec<_>, _>>()?;
            let messages = NonEmpty::from_vec(messages).expect("same length as events");
            publish_with_retry(
                &self.publisher,
                &messages,
                &self.config,
                &self.metrics,
                cancel,
            )
            .await?;
            self.checkpoints
                .save(&self.name, events.last().metadata.seq_nr)
                .await?;
            self.metrics.add_published(events.len() as u64);
            total += events.len();
            tracing::debug!(relay = %self.name, count = events.len(), "published journal batch");
        }
        self.metrics.set_lag(total as u64);
        self.metrics.add_pass();
        Ok(total)
    }

    /// Runs the relay until `cancel` fires.
    pub async fn run(&self, cancel: CancellationToken) -> Result<(), RelayError> {
        let wakeups = std::mem::take(&mut *self.wakeups.lock().unwrap());
        let inner = cancel.clone();
        run_loop(&self.config, wakeups, cancel, || self.pass(&inner)).await
    }

    /// Runs the relay while holding a leader lock (see
    /// [`OutboxRelay::run_as_leader`](crate::OutboxRelay::run_as_leader)).
    pub async fn run_as_leader(
        &self,
        lock: LeaderLock,
        cancel: CancellationToken,
    ) -> Result<(), RelayError> {
        let mut wakeups = Some(std::mem::take(&mut *self.wakeups.lock().unwrap()));
        loop {
            if cancel.is_cancelled() {
                return Ok(());
            }
            match lock.try_acquire().await? {
                Some(mut guard) => {
                    tracing::info!(relay = %self.name, "became the journal relay leader");
                    let leader_cancel = cancel.child_token();
                    let streams = wakeups.take().unwrap_or_default();
                    let inner = leader_cancel.clone();
                    let relay = run_loop(&self.config, streams, leader_cancel.clone(), || {
                        self.pass(&inner)
                    });
                    let outcome = tokio::select! {
                        r = relay => Some(r),
                        _ = guard.lost() => None,
                    };
                    match outcome {
                        Some(result) => {
                            guard.release().await;
                            return result;
                        }
                        None => {
                            tracing::warn!(relay = %self.name, "leader lock lost, standing by");
                            leader_cancel.cancel();
                            guard.release().await;
                        }
                    }
                }
                None => {
                    tokio::select! {
                        _ = cancel.cancelled() => return Ok(()),
                        _ = tokio::time::sleep(self.config.leader_retry_interval) => {}
                    }
                }
            }
        }
    }
}
