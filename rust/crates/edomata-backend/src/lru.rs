//! In-memory caches.

use std::collections::{BTreeMap, HashMap};
use std::hash::Hash;
use std::sync::Mutex;

use async_trait::async_trait;

/// A key/value cache. Mirrors Scala's `Cache[F, I, T]`.
#[async_trait]
pub trait Cache<K, V>: Send + Sync {
    /// Adds or replaces an entry. Returns the entry that was evicted to make
    /// room, if any.
    async fn add(&self, key: K, value: V) -> Option<(K, V)>;

    /// Replaces the entry for `key` only if `pred` holds for the current
    /// value, or adds it when absent. Returns the evicted entry, if any.
    async fn replace(
        &self,
        key: K,
        value: V,
        pred: &(dyn for<'v> Fn(&'v V) -> bool + Send + Sync),
    ) -> Option<(K, V)>;

    /// Reads an entry, marking it as recently used.
    async fn get(&self, key: &K) -> Option<V>;
}

/// A least-recently-used cache with a fixed maximum size.
///
/// `add` and `get` take `O(log(size))` time; iteration by usage takes
/// `O(size)`. The cache is safe to share between tasks.
///
/// ```
/// use edomata_backend::LruCache;
///
/// let cache = LruCache::new(2);
/// assert_eq!(cache.insert(1, "a"), None);
/// assert_eq!(cache.insert(2, "b"), None);
/// assert_eq!(cache.lookup(&1), Some("a"));       // 1 is now the most recent
/// assert_eq!(cache.insert(3, "c"), Some((2, "b"))); // 2 was least recently used
/// assert_eq!(cache.values_by_usage(), vec!["c", "a"]);
/// ```
#[derive(Debug)]
pub struct LruCache<K, V> {
    inner: Mutex<Inner<K, V>>,
    max_size: usize,
}

#[derive(Debug)]
struct Inner<K, V> {
    /// key -> (value, usage tick)
    values: HashMap<K, (V, u64)>,
    /// usage tick -> key, ascending: the first entry is the least recently
    /// used one.
    order: BTreeMap<u64, K>,
    tick: u64,
}

impl<K: Hash + Eq + Clone, V: Clone> LruCache<K, V> {
    /// Creates a cache holding at most `max_size` entries.
    pub fn new(max_size: usize) -> Self {
        Self {
            inner: Mutex::new(Inner {
                values: HashMap::new(),
                order: BTreeMap::new(),
                tick: 0,
            }),
            max_size,
        }
    }

    /// The maximum number of entries.
    pub fn max_size(&self) -> usize {
        self.max_size
    }

    /// The current number of entries.
    pub fn len(&self) -> usize {
        self.lock().values.len()
    }

    /// Whether the cache is empty.
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Adds or replaces an entry, returning the evicted entry if any.
    pub fn insert(&self, key: K, value: V) -> Option<(K, V)> {
        self.insert_if(key, value, |_| true)
    }

    /// Replaces the entry for `key` if `pred` holds for its current value,
    /// or adds it when absent. Returns the evicted entry if any.
    pub fn insert_if(&self, key: K, value: V, pred: impl FnOnce(&V) -> bool) -> Option<(K, V)> {
        let mut inner = self.lock();
        match inner.values.get(&key) {
            Some((existing, _)) if pred(existing) => {
                inner.touch(&key);
                if let Some((v, _)) = inner.values.get_mut(&key) {
                    *v = value;
                }
            }
            Some(_) => {}
            None => {
                let tick = inner.next_tick();
                inner.order.insert(tick, key.clone());
                inner.values.insert(key, (value, tick));
            }
        }
        inner.evict(self.max_size)
    }

    /// Reads an entry, marking it as recently used.
    pub fn lookup(&self, key: &K) -> Option<V> {
        let mut inner = self.lock();
        if inner.values.contains_key(key) {
            inner.touch(key);
        }
        inner.values.get(key).map(|(v, _)| v.clone())
    }

    /// Reads an entry without changing its usage.
    pub fn peek(&self, key: &K) -> Option<V> {
        self.lock().values.get(key).map(|(v, _)| v.clone())
    }

    /// The most recently used value.
    pub fn first_value(&self) -> Option<V> {
        let inner = self.lock();
        inner
            .order
            .values()
            .next_back()
            .and_then(|k| inner.values.get(k))
            .map(|(v, _)| v.clone())
    }

    /// The least recently used value.
    pub fn last_value(&self) -> Option<V> {
        let inner = self.lock();
        inner
            .order
            .values()
            .next()
            .and_then(|k| inner.values.get(k))
            .map(|(v, _)| v.clone())
    }

    /// All entries, most recently used first.
    pub fn by_usage(&self) -> Vec<(K, V)> {
        let inner = self.lock();
        inner
            .order
            .values()
            .rev()
            .filter_map(|k| inner.values.get(k).map(|(v, _)| (k.clone(), v.clone())))
            .collect()
    }

    /// All values, most recently used first.
    pub fn values_by_usage(&self) -> Vec<V> {
        self.by_usage().into_iter().map(|(_, v)| v).collect()
    }

    /// All entries, in no particular order.
    pub fn entries(&self) -> Vec<(K, V)> {
        self.lock()
            .values
            .iter()
            .map(|(k, (v, _))| (k.clone(), v.clone()))
            .collect()
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, Inner<K, V>> {
        // A poisoned lock only happens if a panic occurred while holding it;
        // the data is still consistent because every mutation is applied
        // before releasing the guard, so recovering is safe.
        self.inner
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }
}

impl<K: Hash + Eq + Clone, V> Inner<K, V> {
    fn next_tick(&mut self) -> u64 {
        self.tick += 1;
        self.tick
    }

    fn touch(&mut self, key: &K) {
        let tick = self.next_tick();
        if let Some((_, old)) = self.values.get_mut(key) {
            self.order.remove(old);
            *old = tick;
            self.order.insert(tick, key.clone());
        }
    }

    fn evict(&mut self, max_size: usize) -> Option<(K, V)> {
        if self.values.len() > max_size {
            let (_, key) = self.order.pop_first()?;
            let (value, _) = self.values.remove(&key)?;
            Some((key, value))
        } else {
            None
        }
    }
}

#[async_trait]
impl<K, V> Cache<K, V> for LruCache<K, V>
where
    K: Hash + Eq + Clone + Send + Sync,
    V: Clone + Send + Sync,
{
    async fn add(&self, key: K, value: V) -> Option<(K, V)> {
        self.insert(key, value)
    }

    async fn replace(
        &self,
        key: K,
        value: V,
        pred: &(dyn for<'v> Fn(&'v V) -> bool + Send + Sync),
    ) -> Option<(K, V)> {
        self.insert_if(key, value, pred)
    }

    async fn get(&self, key: &K) -> Option<V> {
        self.lookup(key)
    }
}
