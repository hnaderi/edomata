//! Snapshots: caching folded aggregates in memory and persisting them.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use async_trait::async_trait;
use tokio::sync::{Notify, mpsc};
use tokio::task::JoinHandle;

use super::ValidState;
use crate::{BackendError, LruCache, Payload, StreamId};

/// Reads snapshots.
#[async_trait]
pub trait SnapshotReader<S>: Send + Sync {
    /// Reads the snapshot of an aggregate.
    ///
    /// This might involve reading from disk, and may return a version that
    /// lags behind the in-memory one because of buffering.
    async fn get(&self, id: &str) -> Result<Option<ValidState<S>>, BackendError>;

    /// Reads the snapshot from fast storage (a cache) only. Always returns
    /// the latest cached version, or `None` when not cached.
    async fn get_fast(&self, id: &str) -> Option<ValidState<S>>;
}

/// Reads and writes snapshots.
#[async_trait]
pub trait SnapshotStore<S>: SnapshotReader<S> {
    /// Records the latest state of an aggregate. Older versions never
    /// overwrite newer ones.
    async fn put(&self, id: &str, state: ValidState<S>) -> Result<(), BackendError>;

    /// Releases resources, flushing pending snapshots if the store does so.
    async fn close(&self) -> Result<(), BackendError> {
        Ok(())
    }
}

/// Views a snapshot store as a reader (an explicit upcast, which keeps the
/// crate compatible with the declared MSRV).
pub fn as_reader<S: Payload>(store: Arc<dyn SnapshotStore<S>>) -> Arc<dyn SnapshotReader<S>> {
    Arc::new(ReaderOf(store))
}

struct ReaderOf<S>(Arc<dyn SnapshotStore<S>>);

#[async_trait]
impl<S: Payload> SnapshotReader<S> for ReaderOf<S> {
    async fn get(&self, id: &str) -> Result<Option<ValidState<S>>, BackendError> {
        self.0.get(id).await
    }

    async fn get_fast(&self, id: &str) -> Option<ValidState<S>> {
        self.0.get_fast(id).await
    }
}

/// A snapshot to persist: stream id and folded state.
pub type SnapshotItem<S> = (StreamId, ValidState<S>);

/// Durable snapshot storage, written in batches.
#[async_trait]
pub trait SnapshotPersistence<S>: Send + Sync {
    /// Reads a persisted snapshot.
    async fn get(&self, id: &str) -> Result<Option<ValidState<S>>, BackendError>;
    /// Persists a batch of snapshots. Implementations should [`dedup`] the
    /// batch so that only the latest version of each aggregate is written.
    async fn put(&self, items: Vec<SnapshotItem<S>>) -> Result<(), BackendError>;
}

/// Keeps only the highest version of each aggregate in a batch, preserving
/// the order of first appearance.
pub fn dedup<S>(items: Vec<SnapshotItem<S>>) -> Vec<SnapshotItem<S>> {
    let mut index: HashMap<StreamId, usize> = HashMap::new();
    let mut out: Vec<SnapshotItem<S>> = Vec::new();
    for (id, state) in items {
        match index.get(&id) {
            Some(&i) => {
                if out[i].1.version <= state.version {
                    out[i].1 = state;
                }
            }
            None => {
                index.insert(id.clone(), out.len());
                out.push((id, state));
            }
        }
    }
    out
}

/// An in-memory, bounded [`SnapshotStore`] (an LRU cache).
#[derive(Debug)]
pub struct InMemorySnapshotStore<S> {
    cache: LruCache<StreamId, ValidState<S>>,
}

impl<S: Clone> InMemorySnapshotStore<S> {
    /// Creates a store caching at most `size` aggregates.
    pub fn new(size: usize) -> Self {
        Self {
            cache: LruCache::new(size),
        }
    }
}

#[async_trait]
impl<S: Payload> SnapshotReader<S> for InMemorySnapshotStore<S> {
    async fn get(&self, id: &str) -> Result<Option<ValidState<S>>, BackendError> {
        Ok(self.cache.lookup(&id.to_owned()))
    }

    async fn get_fast(&self, id: &str) -> Option<ValidState<S>> {
        self.cache.lookup(&id.to_owned())
    }
}

#[async_trait]
impl<S: Payload> SnapshotStore<S> for InMemorySnapshotStore<S> {
    async fn put(&self, id: &str, state: ValidState<S>) -> Result<(), BackendError> {
        let version = state.version;
        self.cache
            .insert_if(id.to_owned(), state, |existing| existing.version < version);
        Ok(())
    }
}

/// Configuration of a [`PersistedSnapshotStore`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PersistedSnapshotConfig {
    /// Number of aggregates kept in memory.
    pub size: usize,
    /// Evicted snapshots are persisted in batches of at most this size...
    pub max_buffer: usize,
    /// ...or after this delay, whichever comes first.
    pub max_wait: Duration,
    /// Whether every cached snapshot is persisted on [`SnapshotStore::close`].
    pub flush_on_exit: bool,
}

impl Default for PersistedSnapshotConfig {
    fn default() -> Self {
        Self {
            size: 1000,
            max_buffer: 100,
            max_wait: Duration::from_secs(60),
            flush_on_exit: true,
        }
    }
}

/// A [`SnapshotStore`] that caches snapshots in memory and persists the
/// ones evicted from the cache asynchronously, in batches, with retries.
///
/// A background task drains the queue of evicted snapshots. It is started
/// by [`PersistedSnapshotStore::new`] (which therefore needs a Tokio
/// runtime) and stopped by [`SnapshotStore::close`], which also flushes the
/// whole cache when `flush_on_exit` is set. Dropping the store without
/// closing it aborts the task without flushing.
pub struct PersistedSnapshotStore<S> {
    cache: Arc<LruCache<StreamId, ValidState<S>>>,
    persistence: Arc<dyn SnapshotPersistence<S>>,
    queue: mpsc::Sender<SnapshotItem<S>>,
    shutdown: Arc<Notify>,
    worker: Mutex<Option<JoinHandle<()>>>,
    config: PersistedSnapshotConfig,
}

impl<S> std::fmt::Debug for PersistedSnapshotStore<S> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PersistedSnapshotStore")
            .field("config", &self.config)
            .finish_non_exhaustive()
    }
}

impl<S: Payload> PersistedSnapshotStore<S> {
    /// Creates the store and starts its background persister.
    pub fn new(
        persistence: Arc<dyn SnapshotPersistence<S>>,
        config: PersistedSnapshotConfig,
    ) -> Self {
        let cache = Arc::new(LruCache::new(config.size));
        let (tx, rx) = mpsc::channel(config.max_buffer.max(1));
        let shutdown = Arc::new(Notify::new());
        let worker = tokio::spawn(persister(
            rx,
            Arc::clone(&persistence),
            config,
            Arc::clone(&shutdown),
        ));
        Self {
            cache,
            persistence,
            queue: tx,
            shutdown,
            worker: Mutex::new(Some(worker)),
            config,
        }
    }

    /// Persists every cached snapshot, most recently used first.
    pub async fn flush(&self) -> Result<(), BackendError> {
        let chunk = self.config.size.clamp(1, 1000);
        let items = self.cache.by_usage();
        for batch in items.chunks(chunk) {
            self.persistence.put(batch.to_vec()).await?;
        }
        Ok(())
    }
}

impl<S> PersistedSnapshotStore<S> {
    fn take_worker(&self) -> Option<JoinHandle<()>> {
        self.worker
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .take()
    }
}

impl<S> Drop for PersistedSnapshotStore<S> {
    fn drop(&mut self) {
        if let Some(worker) = self.take_worker() {
            worker.abort();
        }
    }
}

#[async_trait]
impl<S: Payload> SnapshotReader<S> for PersistedSnapshotStore<S> {
    async fn get(&self, id: &str) -> Result<Option<ValidState<S>>, BackendError> {
        match self.get_fast(id).await {
            Some(cached) => Ok(Some(cached)),
            None => self.persistence.get(id).await,
        }
    }

    async fn get_fast(&self, id: &str) -> Option<ValidState<S>> {
        self.cache.lookup(&id.to_owned())
    }
}

#[async_trait]
impl<S: Payload> SnapshotStore<S> for PersistedSnapshotStore<S> {
    async fn put(&self, id: &str, state: ValidState<S>) -> Result<(), BackendError> {
        let version = state.version;
        if let Some(evicted) = self
            .cache
            .insert_if(id.to_owned(), state, |existing| existing.version < version)
        {
            // A full queue drops the snapshot, like Scala's `Queue.dropping`.
            let _ = self.queue.try_send(evicted);
        }
        Ok(())
    }

    async fn close(&self) -> Result<(), BackendError> {
        if let Some(worker) = self.take_worker() {
            self.shutdown.notify_one();
            let _ = worker.await;
        }
        if self.config.flush_on_exit {
            self.flush().await?;
        }
        Ok(())
    }
}

/// Drains evicted snapshots: groups them by `max_buffer` / `max_wait` and
/// persists every group, retrying up to 3 times with delays of 1 s and 2 s.
async fn persister<S: Payload>(
    mut rx: mpsc::Receiver<SnapshotItem<S>>,
    persistence: Arc<dyn SnapshotPersistence<S>>,
    config: PersistedSnapshotConfig,
    shutdown: Arc<Notify>,
) {
    loop {
        let first = tokio::select! {
            item = rx.recv() => match item {
                Some(item) => item,
                None => return,
            },
            () = shutdown.notified() => return,
        };
        let mut batch = vec![first];
        let deadline = tokio::time::sleep(config.max_wait);
        let mut deadline = std::pin::pin!(deadline);
        while batch.len() < config.max_buffer {
            tokio::select! {
                item = rx.recv() => match item {
                    Some(item) => batch.push(item),
                    None => break,
                },
                () = &mut deadline => break,
                () = shutdown.notified() => return,
            }
        }
        persist_with_retry(&*persistence, batch).await;
    }
}

async fn persist_with_retry<S: Payload>(
    persistence: &dyn SnapshotPersistence<S>,
    batch: Vec<SnapshotItem<S>>,
) {
    let mut delay = Duration::from_secs(1);
    for attempt in 1..=3 {
        match persistence.put(batch.clone()).await {
            Ok(()) => return,
            Err(error) if attempt < 3 => {
                tracing::warn!(%error, attempt, "persisting snapshots failed, retrying");
                tokio::time::sleep(delay).await;
                delay *= 2;
            }
            Err(error) => {
                tracing::error!(%error, "persisting snapshots failed, dropping batch");
            }
        }
    }
}
