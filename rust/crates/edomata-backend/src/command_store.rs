//! Command idempotency store.

use std::collections::HashSet;
use std::sync::Mutex;

use async_trait::async_trait;

use crate::{BackendError, LruCache};

/// Remembers which command ids have already been handled, so that a command
/// delivered twice is not processed twice. Mirrors Scala's `CommandStore`.
#[async_trait]
pub trait CommandStore: Send + Sync {
    /// Records a handled command.
    async fn append(&self, command_id: &str) -> Result<(), BackendError>;
    /// Whether the command was already handled.
    async fn contains(&self, command_id: &str) -> Result<bool, BackendError>;
}

/// A bounded in-memory [`CommandStore`]: keeps the `size` most recently
/// appended command ids.
///
/// ```
/// # futures::executor::block_on(async {
/// use edomata_backend::{CommandStore, InMemoryCommandStore};
///
/// let store = InMemoryCommandStore::new(1);
/// store.append("a").await.unwrap();
/// store.append("b").await.unwrap();
/// assert_eq!(store.contains("a").await.unwrap(), false);
/// assert_eq!(store.contains("b").await.unwrap(), true);
/// # });
/// ```
#[derive(Debug)]
pub struct InMemoryCommandStore {
    cache: LruCache<String, ()>,
    set: Mutex<HashSet<String>>,
}

impl InMemoryCommandStore {
    /// Creates a store remembering at most `size` command ids.
    pub fn new(size: usize) -> Self {
        Self {
            cache: LruCache::new(size),
            set: Mutex::new(HashSet::new()),
        }
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, HashSet<String>> {
        self.set
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }
}

#[async_trait]
impl CommandStore for InMemoryCommandStore {
    async fn append(&self, command_id: &str) -> Result<(), BackendError> {
        let evicted = self.cache.insert(command_id.to_owned(), ());
        let mut set = self.lock();
        if let Some((old, ())) = evicted {
            set.remove(&old);
        }
        set.insert(command_id.to_owned());
        Ok(())
    }

    async fn contains(&self, command_id: &str) -> Result<bool, BackendError> {
        Ok(self.lock().contains(command_id))
    }
}
