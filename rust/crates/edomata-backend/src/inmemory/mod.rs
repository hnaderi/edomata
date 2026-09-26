//! In-memory storage driver, for tests and prototypes.
//!
//! [`InMemoryDriver`] implements both the event-sourcing and the CQRS
//! [`StorageDriver`](crate::eventsourcing::StorageDriver) traits. It needs
//! no codec (`Codec<T> = ()`), so backends are built with `build_default()`.
//! Data lives in the process and is lost when the store is dropped.
//!
//! Optimistic concurrency and command idempotency follow the PostgreSQL
//! drivers exactly: a duplicate `(stream, version)` or command id fails
//! with [`BackendError::VersionConflict`](crate::BackendError::VersionConflict).

mod cqrs;
mod eventsourcing;

use std::any::Any;
use std::sync::{Arc, Mutex, MutexGuard};

use chrono::{DateTime, Utc};
use edomata_core::{MessageMetadata, NonEmpty};

pub use self::cqrs::{InMemoryNotificationHandler, InMemoryStateStore};
pub use self::eventsourcing::{InMemoryEventStore, InMemorySnapshotPersistence};
use crate::{OutboxItem, SeqNr, StreamId};

/// The in-memory driver. See the [module documentation](self).
#[derive(Clone, Default)]
pub struct InMemoryDriver {
    store: Option<Arc<dyn Any + Send + Sync>>,
    snapshots: Option<Arc<dyn Any + Send + Sync>>,
}

impl std::fmt::Debug for InMemoryDriver {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("InMemoryDriver")
            .field("seeded", &self.store.is_some())
            .finish()
    }
}

impl InMemoryDriver {
    /// A driver whose `build` creates a fresh, empty store.
    pub fn new() -> Self {
        Self::default()
    }

    /// A driver whose `build` uses the given (possibly pre-populated) event
    /// store, and whose `snapshot` uses that store's snapshot persistence.
    /// The type parameters of the store must match the ones the backend is
    /// built with.
    pub fn with_event_store<S, E, N>(store: Arc<InMemoryEventStore<S, E, N>>) -> Self
    where
        S: Send + Sync + 'static,
        E: Send + Sync + 'static,
        N: Send + Sync + 'static,
    {
        let snapshots: Arc<dyn Any + Send + Sync> = store.snapshots();
        Self {
            store: Some(store),
            snapshots: Some(snapshots),
        }
    }

    /// A driver whose `build` uses the given (possibly pre-populated) state
    /// store. The type parameters of the store must match the ones the
    /// backend is built with.
    pub fn with_state_store<S, N>(store: Arc<InMemoryStateStore<S, N>>) -> Self
    where
        S: Send + Sync + 'static,
        N: Send + Sync + 'static,
    {
        Self {
            store: Some(store),
            snapshots: None,
        }
    }

    fn snapshots_of<S: Send + Sync + 'static>(
        &self,
    ) -> Option<Arc<InMemorySnapshotPersistence<S>>> {
        self.snapshots.as_ref().and_then(|s| {
            Arc::clone(s)
                .downcast::<InMemorySnapshotPersistence<S>>()
                .ok()
        })
    }

    fn store_of<T: Send + Sync + 'static>(&self) -> Result<Arc<T>, crate::BackendError> {
        match &self.store {
            None => Err(crate::BackendError::persistence("no seeded store")),
            Some(store) => Arc::clone(store).downcast::<T>().map_err(|_| {
                crate::BackendError::persistence(
                    "the seeded in-memory store does not match the backend's payload types",
                )
            }),
        }
    }
}

/// An outbox row.
#[derive(Clone, Debug)]
struct OutboxRow<N> {
    item: OutboxItem<N>,
    published: Option<DateTime<Utc>>,
}

/// Outbox and command tables shared by both stores.
#[derive(Debug)]
struct Tables<N> {
    outbox: Vec<OutboxRow<N>>,
    commands: std::collections::HashMap<String, (StreamId, DateTime<Utc>)>,
    next_outbox_seq: SeqNr,
}

impl<N> Default for Tables<N> {
    fn default() -> Self {
        Self {
            outbox: Vec::new(),
            commands: std::collections::HashMap::new(),
            next_outbox_seq: 1,
        }
    }
}

impl<N: Clone> Tables<N> {
    fn insert_outbox(
        &mut self,
        stream: &str,
        time: DateTime<Utc>,
        metadata: &MessageMetadata,
        notifications: impl IntoIterator<Item = N>,
    ) {
        for data in notifications {
            let seq_nr = self.next_outbox_seq;
            self.next_outbox_seq += 1;
            self.outbox.push(OutboxRow {
                item: OutboxItem {
                    seq_nr,
                    stream_id: stream.to_owned(),
                    time,
                    data,
                    metadata: metadata.clone(),
                },
                published: None,
            });
        }
    }

    fn seed_outbox(&mut self, items: impl IntoIterator<Item = OutboxItem<N>>) {
        for item in items {
            self.next_outbox_seq = self.next_outbox_seq.max(item.seq_nr + 1);
            self.outbox.push(OutboxRow {
                item,
                published: None,
            });
        }
    }

    fn unpublished(&self) -> Vec<OutboxItem<N>> {
        let mut items: Vec<_> = self
            .outbox
            .iter()
            .filter(|row| row.published.is_none())
            .map(|row| row.item.clone())
            .collect();
        items.sort_by_key(|item| item.seq_nr);
        items
    }

    fn mark_published(&mut self, items: &NonEmpty<OutboxItem<N>>, now: DateTime<Utc>) {
        for row in &mut self.outbox {
            if items.iter().any(|item| item.seq_nr == row.item.seq_nr) {
                row.published = Some(now);
            }
        }
    }

    fn insert_command(&mut self, id: &str, address: &str, time: DateTime<Utc>) -> bool {
        if self.commands.contains_key(id) {
            return false;
        }
        self.commands
            .insert(id.to_owned(), (address.to_owned(), time));
        true
    }
}

fn lock<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
    m.lock().unwrap_or_else(std::sync::PoisonError::into_inner)
}
