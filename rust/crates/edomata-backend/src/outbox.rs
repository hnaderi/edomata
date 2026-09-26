//! Transactional outbox: reading items and consuming them.

use std::future::Future;

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use edomata_core::{MessageMetadata, NonEmpty};
use futures::stream::BoxStream;
use futures::{Stream, StreamExt, TryStreamExt};

use crate::{BackendError, SeqNr, StreamId};

/// A notification waiting in the outbox.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct OutboxItem<N> {
    /// Global sequence number of the item.
    pub seq_nr: SeqNr,
    /// Aggregate that published the notification.
    pub stream_id: StreamId,
    /// When the notification was written.
    pub time: DateTime<Utc>,
    /// The notification payload.
    pub data: N,
    /// Correlation and causation of the command that produced it.
    pub metadata: MessageMetadata,
}

impl<N> OutboxItem<N> {
    /// Changes the payload.
    pub fn map<M, F: FnOnce(N) -> M>(self, f: F) -> OutboxItem<M> {
        OutboxItem {
            seq_nr: self.seq_nr,
            stream_id: self.stream_id,
            time: self.time,
            data: f(self.data),
            metadata: self.metadata,
        }
    }
}

/// Reads unpublished outbox items and marks them as sent.
#[async_trait]
pub trait OutboxReader<N>: Send + Sync {
    /// Streams every item that has not been marked as sent, by ascending
    /// sequence number.
    fn read(&self) -> BoxStream<'_, Result<OutboxItem<N>, BackendError>>;

    /// Marks all the given items as sent.
    async fn mark_all_as_sent(&self, items: &NonEmpty<OutboxItem<N>>) -> Result<(), BackendError>;

    /// Marks a single item as sent.
    async fn mark_as_sent(&self, item: &OutboxItem<N>) -> Result<(), BackendError>
    where
        N: Clone + Send + Sync,
    {
        self.mark_all_as_sent(&NonEmpty::new(item.clone())).await
    }
}

/// Default number of items handled before they are marked as sent.
pub const DEFAULT_OUTBOX_BATCH_SIZE: usize = 100;

/// Consumes outbox items: runs a handler on every unpublished item, then
/// marks the batch as sent. Mirrors Scala's `OutboxConsumer`.
///
/// The consumer processes the outbox once immediately, then again every
/// time `signal` yields (typically the backend's outbox update signal), and
/// completes when `signal` ends.
///
/// Items are processed in batches of at most [`DEFAULT_OUTBOX_BATCH_SIZE`];
/// a batch is marked as sent only after the handler succeeded for every
/// item in it, so delivery is at-least-once.
#[derive(Clone, Copy, Debug)]
pub struct OutboxConsumer {
    batch_size: usize,
}

impl Default for OutboxConsumer {
    fn default() -> Self {
        Self::new()
    }
}

impl OutboxConsumer {
    /// A consumer with the default batch size.
    pub fn new() -> Self {
        Self {
            batch_size: DEFAULT_OUTBOX_BATCH_SIZE,
        }
    }

    /// Sets the number of items marked as sent at once (at least 1).
    pub fn with_batch_size(self, batch_size: usize) -> Self {
        Self {
            batch_size: batch_size.max(1),
        }
    }

    /// Runs `handler` on every unpublished item of `reader`, once now and
    /// then after each element of `signal`.
    pub async fn run<N, S, F, Fut>(
        self,
        reader: &dyn OutboxReader<N>,
        signal: S,
        mut handler: F,
    ) -> Result<(), BackendError>
    where
        N: Clone + Send + Sync,
        S: Stream<Item = ()> + Send,
        F: FnMut(OutboxItem<N>) -> Fut + Send,
        Fut: Future<Output = Result<(), BackendError>> + Send,
    {
        let mut signal = std::pin::pin!(signal);
        loop {
            self.consume_once(reader, &mut handler).await?;
            if signal.next().await.is_none() {
                return Ok(());
            }
        }
    }

    /// Runs `handler` on every unpublished item of `reader` once.
    pub async fn consume_once<N, F, Fut>(
        self,
        reader: &dyn OutboxReader<N>,
        handler: &mut F,
    ) -> Result<(), BackendError>
    where
        N: Clone + Send + Sync,
        F: FnMut(OutboxItem<N>) -> Fut + Send,
        Fut: Future<Output = Result<(), BackendError>> + Send,
    {
        let mut batches = reader.read().try_chunks(self.batch_size);
        while let Some(batch) = batches.next().await {
            let batch = batch.map_err(|e| e.1)?;
            for item in batch.iter().cloned() {
                handler(item).await?;
            }
            if let Some(items) = NonEmpty::from_vec(batch) {
                reader.mark_all_as_sent(&items).await?;
            }
        }
        Ok(())
    }
}
