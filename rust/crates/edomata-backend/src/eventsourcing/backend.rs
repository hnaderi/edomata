//! The event-sourcing [`Backend`] and its builder.

use std::sync::Arc;

use edomata_core::{DomainDsl, DomainModel, Edomaton, RequestContext};

use super::{
    CachedRepository, CommandHandler, InMemorySnapshotStore, JournalReader, NotificationsConsumer,
    PersistedSnapshotConfig, PersistedSnapshotStore, RepositoryReader, SnapshotStore, Storage,
    StorageDriver,
};
use crate::{
    BackendError, CommandStore, DomainService, InMemoryCommandStore, OutboxConsumer, OutboxItem,
    OutboxReader, Payload, RetryConfig, SharedModel,
};

/// A ready-to-use event-sourcing backend for one aggregate type.
///
/// Obtain one with [`Backend::builder`]:
///
/// ```
/// # use edomata_core::*;
/// # use edomata_backend::eventsourcing::Backend;
/// # use edomata_backend::inmemory::InMemoryDriver;
/// # struct Counter;
/// # impl DomainModel for Counter {
/// #     type State = i32; type Event = i32; type Rejection = String;
/// #     fn initial(&self) -> i32 { 0 }
/// #     fn transition(&self, e: &i32, s: i32) -> Result<i32, NonEmpty<String>> { Ok(s + e) }
/// # }
/// # tokio::runtime::Runtime::new().unwrap().block_on(async {
/// let dsl = Counter.dsl::<i32, String>();
/// let backend = Backend::builder(Counter, dsl)
///     .driver(InMemoryDriver::new())
///     .build_default()
///     .await
///     .unwrap();
///
/// let service = backend.compile(dsl.router(move |by| dsl.accept(by)));
/// let cmd = CommandMessage::new("cmd-1", chrono::Utc::now(), "counter-1", 5);
/// assert_eq!(service(cmd).await.unwrap(), Ok(()));
/// let state = backend.repository().get("counter-1").await.unwrap();
/// assert_eq!(state.as_valid().map(|v| v.state), Some(5));
/// # });
/// ```
pub struct Backend<S, E, R, N> {
    handler: CommandHandler<S, E, R, N>,
    outbox: Arc<dyn OutboxReader<N>>,
    journal: Arc<dyn JournalReader<E>>,
    repository: Arc<dyn RepositoryReader<S, E, R>>,
    updates: Arc<dyn NotificationsConsumer>,
    snapshot: Arc<dyn SnapshotStore<S>>,
}

impl<S, E, R, N> Clone for Backend<S, E, R, N> {
    fn clone(&self) -> Self {
        Self {
            handler: self.handler.clone(),
            outbox: Arc::clone(&self.outbox),
            journal: Arc::clone(&self.journal),
            repository: Arc::clone(&self.repository),
            updates: Arc::clone(&self.updates),
            snapshot: Arc::clone(&self.snapshot),
        }
    }
}

impl<S: Payload, E: Payload, R: Payload, N: Payload> Backend<S, E, R, N> {
    /// Starts building a backend for `model`. The `domain` marker fixes the
    /// notification type (its command type is irrelevant here).
    pub fn builder<M, C>(
        model: M,
        domain: DomainDsl<C, S, E, R, N>,
    ) -> PartialBackendBuilder<S, E, R, N>
    where
        M: DomainModel<State = S, Event = E, Rejection = R> + Send + Sync + 'static,
    {
        let _ = domain;
        PartialBackendBuilder {
            model: Arc::new(model),
            _n: std::marker::PhantomData,
        }
    }

    /// Compiles a domain program into a service.
    pub fn compile<C: Payload>(
        &self,
        app: Edomaton<RequestContext<C, S>, R, E, N, ()>,
    ) -> DomainService<C, R> {
        self.handler.compile(app)
    }

    /// The outbox.
    pub fn outbox(&self) -> &Arc<dyn OutboxReader<N>> {
        &self.outbox
    }

    /// The journal.
    pub fn journal(&self) -> &Arc<dyn JournalReader<E>> {
        &self.journal
    }

    /// The read side.
    pub fn repository(&self) -> &Arc<dyn RepositoryReader<S, E, R>> {
        &self.repository
    }

    /// Update signals.
    pub fn updates(&self) -> &Arc<dyn NotificationsConsumer> {
        &self.updates
    }

    /// Consumes the outbox with `handler`, now and whenever new items are
    /// published, until the backend is dropped. See [`OutboxConsumer`].
    pub async fn consume_outbox<F, Fut>(
        &self,
        consumer: OutboxConsumer,
        handler: F,
    ) -> Result<(), BackendError>
    where
        F: FnMut(OutboxItem<N>) -> Fut + Send,
        Fut: std::future::Future<Output = Result<(), BackendError>> + Send,
    {
        consumer
            .run(self.outbox.as_ref(), self.updates.outbox(), handler)
            .await
    }

    /// Releases resources: flushes persisted snapshots when configured.
    pub async fn close(&self) -> Result<(), BackendError> {
        self.snapshot.close().await
    }
}

/// First step of [`Backend::builder`]: choose a storage driver.
pub struct PartialBackendBuilder<S, E, R, N> {
    model: SharedModel<S, E, R>,
    _n: std::marker::PhantomData<fn() -> N>,
}

impl<S: Payload, E: Payload, R: Payload, N: Payload> PartialBackendBuilder<S, E, R, N> {
    /// Uses `driver` to store this aggregate.
    pub fn driver<D: StorageDriver>(self, driver: D) -> BackendBuilder<S, E, R, N, D> {
        BackendBuilder {
            model: self.model,
            driver,
            snapshot: SnapshotChoice::InMemory(1000),
            command_cache: CommandCacheChoice::InMemory(1000),
            retry: RetryConfig::default(),
            _n: std::marker::PhantomData,
        }
    }
}

enum SnapshotChoice<S, C> {
    InMemory(usize),
    Persisted(C, PersistedSnapshotConfig),
    Custom(Arc<dyn SnapshotStore<S>>),
}

enum CommandCacheChoice {
    Disabled,
    InMemory(usize),
    Custom(Arc<dyn CommandStore>),
}

/// Configures and builds a [`Backend`]. Mirrors Scala's `BackendBuilder`.
///
/// Defaults: an in-memory snapshot cache of 1000 aggregates, an in-memory
/// command cache of 1000 commands, and [`RetryConfig::default`].
pub struct BackendBuilder<S, E, R, N, D: StorageDriver> {
    model: SharedModel<S, E, R>,
    driver: D,
    snapshot: SnapshotChoice<S, D::Codec<S>>,
    command_cache: CommandCacheChoice,
    retry: RetryConfig,
    _n: std::marker::PhantomData<fn() -> N>,
}

impl<S: Payload, E: Payload, R: Payload, N: Payload, D: StorageDriver>
    BackendBuilder<S, E, R, N, D>
{
    /// Persists snapshots through the driver, with default
    /// [`PersistedSnapshotConfig`].
    pub fn persisted_snapshot(self, state_codec: D::Codec<S>) -> Self {
        self.persisted_snapshot_with(state_codec, PersistedSnapshotConfig::default())
    }

    /// Persists snapshots through the driver.
    pub fn persisted_snapshot_with(
        mut self,
        state_codec: D::Codec<S>,
        config: PersistedSnapshotConfig,
    ) -> Self {
        self.snapshot = SnapshotChoice::Persisted(state_codec, config);
        self
    }

    /// Keeps snapshots in memory only (the default, with 1000 entries).
    pub fn in_mem_snapshot(mut self, max_in_mem: usize) -> Self {
        self.snapshot = SnapshotChoice::InMemory(max_in_mem);
        self
    }

    /// Uses a custom snapshot store.
    pub fn with_snapshot(mut self, store: Arc<dyn SnapshotStore<S>>) -> Self {
        self.snapshot = SnapshotChoice::Custom(store);
        self
    }

    /// Disables command caching (every command is loaded from the storage).
    pub fn disable_cache(mut self) -> Self {
        self.command_cache = CommandCacheChoice::Disabled;
        self
    }

    /// Uses a custom command store.
    pub fn with_command_cache(mut self, store: Arc<dyn CommandStore>) -> Self {
        self.command_cache = CommandCacheChoice::Custom(store);
        self
    }

    /// Caches the ids of the last `max_commands_to_cache` commands in
    /// memory.
    pub fn with_command_cache_size(mut self, max_commands_to_cache: usize) -> Self {
        self.command_cache = CommandCacheChoice::InMemory(max_commands_to_cache);
        self
    }

    /// Sets the retry policy for version conflicts.
    pub fn with_retry_config(mut self, retry: RetryConfig) -> Self {
        self.retry = retry;
        self
    }

    /// The configured retry policy.
    pub fn retry_config(&self) -> RetryConfig {
        self.retry
    }

    /// Builds the backend with default codecs.
    pub async fn build_default(self) -> Result<Backend<S, E, R, N>, BackendError>
    where
        D::Codec<E>: Default,
        D::Codec<N>: Default,
    {
        self.build(Default::default(), Default::default()).await
    }

    /// Builds the backend.
    pub async fn build(
        self,
        event_codec: D::Codec<E>,
        notification_codec: D::Codec<N>,
    ) -> Result<Backend<S, E, R, N>, BackendError> {
        let snapshot: Arc<dyn SnapshotStore<S>> = match self.snapshot {
            SnapshotChoice::InMemory(size) => Arc::new(InMemorySnapshotStore::new(size)),
            SnapshotChoice::Persisted(codec, config) => {
                let persistence = self.driver.snapshot(codec).await?;
                Arc::new(PersistedSnapshotStore::new(persistence, config))
            }
            SnapshotChoice::Custom(store) => store,
        };
        let Storage {
            repository,
            reader,
            journal,
            outbox,
            updates,
        } = self
            .driver
            .build(
                Arc::clone(&self.model),
                Arc::clone(&snapshot),
                event_codec,
                notification_codec,
            )
            .await?;
        let repository = match self.command_cache {
            CommandCacheChoice::Disabled => repository,
            CommandCacheChoice::InMemory(size) => Arc::new(CachedRepository::new(
                repository,
                Arc::new(InMemoryCommandStore::new(size)),
                Arc::clone(&snapshot),
            )),
            CommandCacheChoice::Custom(store) => Arc::new(CachedRepository::new(
                repository,
                store,
                Arc::clone(&snapshot),
            )),
        };
        let handler = CommandHandler::with_retry(repository, self.model, self.retry);
        Ok(Backend {
            handler,
            outbox,
            journal,
            repository: reader,
            updates,
            snapshot,
        })
    }
}
