//! Publishing outbox notifications to a broker.

use std::sync::{Arc, Mutex};

use edomata_backend::{OutboxItem, OutboxReader, Payload};
use edomata_core::NonEmpty;
use futures::stream::BoxStream;
use futures::{StreamExt, TryStreamExt};
use tokio_util::sync::CancellationToken;

use crate::postgres::LeaderLock;
use crate::runner::{publish_with_retry, run_loop};
use crate::{
    BrokerMessage, MessageEncoder, MessageKind, Publisher, RelayConfig, RelayError, RelayMetrics,
};

/// Publishes unpublished outbox items to a [`Publisher`] and marks them as
/// sent only after the broker acknowledged them: the `OutboxConsumer` of
/// `edomata-backend` wired to a broker.
///
/// The relay passes over the outbox once when started, then whenever one of
/// its wake-up streams yields (the backend's in-process outbox signal, a
/// PostgreSQL `LISTEN` stream from [`crate::postgres::listen`], ...) and at
/// least every [`RelayConfig::poll_interval`]. Items are published in
/// sequence-number order, in batches of [`RelayConfig::batch_size`];
/// transient failures are retried with exponential backoff and nothing is
/// marked as sent until the batch succeeded, so delivery is at-least-once.
///
/// Several replicas can run the same relay with
/// [`run_as_leader`](Self::run_as_leader): a PostgreSQL advisory lock makes
/// sure only one publishes at a time.
pub struct OutboxRelay<N> {
    reader: Arc<dyn OutboxReader<N>>,
    publisher: Arc<dyn Publisher>,
    encoder: MessageEncoder<N>,
    config: RelayConfig,
    metrics: Arc<RelayMetrics>,
    wakeups: Mutex<Vec<BoxStream<'static, ()>>>,
}

impl<N> std::fmt::Debug for OutboxRelay<N> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("OutboxRelay")
            .field("config", &self.config)
            .finish()
    }
}

impl<N: Payload> OutboxRelay<N> {
    /// A relay from an outbox reader to a publisher.
    pub fn new(
        reader: Arc<dyn OutboxReader<N>>,
        publisher: Arc<dyn Publisher>,
        encoder: MessageEncoder<N>,
        config: RelayConfig,
    ) -> Self {
        Self {
            reader,
            publisher,
            encoder,
            config,
            metrics: Arc::new(RelayMetrics::new()),
            wakeups: Mutex::new(Vec::new()),
        }
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

    /// The configuration.
    pub fn config(&self) -> &RelayConfig {
        &self.config
    }

    /// The message published for an outbox item; its id is
    /// [`BrokerMessage::outbox_id`] of the relay source and the item's
    /// sequence number.
    pub fn message(&self, item: &OutboxItem<N>) -> Result<BrokerMessage, RelayError> {
        Ok(BrokerMessage {
            id: BrokerMessage::outbox_id(&self.config.source, item.seq_nr),
            source: self.config.source.clone(),
            kind: MessageKind::Notification,
            stream_id: item.stream_id.clone(),
            seq_nr: item.seq_nr,
            time: item.time,
            content_type: self.encoder.content_type().to_string(),
            payload: self.encoder.encode(&item.data)?,
            correlation: item.metadata.correlation.clone(),
            causation: item.metadata.causation.clone(),
            extra_headers: Default::default(),
        })
    }

    /// One pass: publishes and marks every pending item. Returns the number
    /// of items published.
    pub async fn relay_once(&self) -> Result<usize, RelayError> {
        self.pass(&CancellationToken::new()).await
    }

    async fn pass(&self, cancel: &CancellationToken) -> Result<usize, RelayError> {
        let mut batches = self.reader.read().try_chunks(self.config.batch_size);
        let mut total = 0usize;
        while let Some(batch) = batches.next().await {
            let batch = batch.map_err(|e| e.1)?;
            let Some(items) = NonEmpty::from_vec(batch) else {
                continue;
            };
            let messages = items
                .iter()
                .map(|item| self.message(item))
                .collect::<Result<Vec<_>, _>>()?;
            let messages = NonEmpty::from_vec(messages).expect("same length as items");
            publish_with_retry(
                &self.publisher,
                &messages,
                &self.config,
                &self.metrics,
                cancel,
            )
            .await?;
            self.reader.mark_all_as_sent(&items).await?;
            self.metrics.add_published(items.len() as u64);
            total += items.len();
            tracing::debug!(source = %self.config.source, count = items.len(), "published outbox batch");
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

    /// Runs the relay while holding the leader lock of the source: a
    /// stand-by replica keeps trying to acquire it every
    /// [`RelayConfig::leader_retry_interval`]; when the lock is lost (the
    /// database connection dropped) the relay stops publishing and goes back
    /// to standing by.
    pub async fn run_as_leader(
        &self,
        lock: LeaderLock,
        cancel: CancellationToken,
    ) -> Result<(), RelayError> {
        let wakeups = std::mem::take(&mut *self.wakeups.lock().unwrap());
        let mut wakeups = Some(wakeups);
        loop {
            if cancel.is_cancelled() {
                return Ok(());
            }
            match lock.try_acquire().await? {
                Some(mut guard) => {
                    tracing::info!(source = %self.config.source, "became the outbox relay leader");
                    let leader_cancel = cancel.child_token();
                    let streams = wakeups.take().unwrap_or_default();
                    let inner = leader_cancel.clone();
                    let relay = run_loop(&self.config, streams, leader_cancel.clone(), || {
                        self.pass(&inner)
                    });
                    let lost = guard.lost();
                    let outcome = tokio::select! {
                        r = relay => Some(r),
                        _ = lost => None,
                    };
                    match outcome {
                        Some(result) => {
                            guard.release().await;
                            return result;
                        }
                        None => {
                            tracing::warn!(source = %self.config.source, "leader lock lost, standing by");
                            leader_cancel.cancel();
                            guard.release().await;
                            // The wake-up streams were consumed by the leader loop;
                            // the polling fallback drives the next term.
                        }
                    }
                }
                None => {
                    tracing::debug!(source = %self.config.source, "another relay is the leader");
                    tokio::select! {
                        _ = cancel.cancelled() => return Ok(()),
                        _ = tokio::time::sleep(self.config.leader_retry_interval) => {}
                    }
                }
            }
        }
    }
}
