//! The CQRS [`Backend`] and its builder.

use std::sync::Arc;

use edomata_core::{CommandMessage, CqrsDomainDsl, CqrsModel, Stomaton};

use super::{
    CachedRepository, CommandHandler, NotificationsConsumer, RepositoryReader, SharedStateModel,
    Storage, StorageDriver,
};
use crate::{
    BackendError, CommandStore, DomainService, InMemoryCommandStore, OutboxConsumer, OutboxItem,
    OutboxReader, Payload, RetryConfig,
};

/// A ready-to-use CQRS backend for one aggregate type.
///
/// ```
/// # use edomata_core::*;
/// # use edomata_backend::cqrs::Backend;
/// # use edomata_backend::inmemory::InMemoryDriver;
/// # struct Tally;
/// # impl CqrsModel for Tally { type State = i32; type Rejection = String; fn initial(&self) -> i32 { 0 } }
/// # tokio::runtime::Runtime::new().unwrap().block_on(async {
/// let dsl = Tally.dsl::<i32, String>();
/// let backend = Backend::builder(Tally, dsl)
///     .driver(InMemoryDriver::new())
///     .build_default()
///     .await
///     .unwrap();
///
/// let service = backend.compile(dsl.router(move |by| dsl.modify(move |s| s + by).void()));
/// let cmd = CommandMessage::new("cmd-1", chrono::Utc::now(), "tally-1", 5);
/// assert_eq!(service(cmd).await.unwrap(), Ok(()));
/// assert_eq!(backend.repository().get("tally-1").await.unwrap().state, 5);
/// # });
/// ```
pub struct Backend<S, N> {
    handler: CommandHandler<S, N>,
    outbox: Arc<dyn OutboxReader<N>>,
    repository: Arc<dyn RepositoryReader<S>>,
    updates: Arc<dyn NotificationsConsumer>,
}

impl<S, N> Clone for Backend<S, N> {
    fn clone(&self) -> Self {
        Self {
            handler: self.handler.clone(),
            outbox: Arc::clone(&self.outbox),
            repository: Arc::clone(&self.repository),
            updates: Arc::clone(&self.updates),
        }
    }
}

impl<S: Payload, N: Payload> Backend<S, N> {
    /// Starts building a backend for `model`. The `domain` marker fixes the
    /// notification type (its command and rejection types are irrelevant
    /// here).
    pub fn builder<M, C, R>(
        model: M,
        domain: CqrsDomainDsl<C, S, R, N>,
    ) -> PartialBackendBuilder<S, N>
    where
        M: CqrsModel<State = S> + Send + Sync + 'static,
    {
        let _ = domain;
        PartialBackendBuilder {
            model: Arc::new(model),
            _n: std::marker::PhantomData,
        }
    }

    /// Compiles a domain program into a service.
    pub fn compile<C: Payload, R: Payload>(
        &self,
        app: Stomaton<CommandMessage<C>, S, R, N, ()>,
    ) -> DomainService<C, R> {
        self.handler.compile(app)
    }

    /// The outbox.
    pub fn outbox(&self) -> &Arc<dyn OutboxReader<N>> {
        &self.outbox
    }

    /// The read side.
    pub fn repository(&self) -> &Arc<dyn RepositoryReader<S>> {
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
}

/// First step of [`Backend::builder`]: choose a storage driver.
pub struct PartialBackendBuilder<S, N> {
    model: SharedStateModel<S>,
    _n: std::marker::PhantomData<fn() -> N>,
}

impl<S: Payload, N: Payload> PartialBackendBuilder<S, N> {
    /// Uses `driver` to store this aggregate.
    pub fn driver<D: StorageDriver>(self, driver: D) -> BackendBuilder<S, N, D> {
        BackendBuilder {
            model: self.model,
            driver,
            command_cache: CommandCacheChoice::InMemory(1000),
            handler: None,
            retry: RetryConfig::default(),
            state_cache_size: 1000,
        }
    }
}

enum CommandCacheChoice {
    Disabled,
    InMemory(usize),
    Custom(Arc<dyn CommandStore>),
}

/// Configures and builds a CQRS [`Backend`]. Mirrors Scala's
/// `cqrs.BackendBuilder`.
///
/// Defaults: an in-memory command cache of 1000 commands, a state cache of
/// 1000 aggregates, and [`RetryConfig::default`].
pub struct BackendBuilder<S, N: 'static, D: StorageDriver> {
    model: SharedStateModel<S>,
    driver: D,
    command_cache: CommandCacheChoice,
    handler: Option<D::Handler<N>>,
    retry: RetryConfig,
    state_cache_size: usize,
}

impl<S: Payload, N: Payload, D: StorageDriver> BackendBuilder<S, N, D> {
    /// Disables command caching (every command is loaded from the storage).
    /// This also disables the state cache.
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

    /// Number of aggregate states cached in memory.
    pub fn with_state_cache_size(mut self, size: usize) -> Self {
        self.state_cache_size = size;
        self
    }

    /// Sets the retry policy for version conflicts.
    pub fn with_retry_config(mut self, retry: RetryConfig) -> Self {
        self.retry = retry;
        self
    }

    /// Runs `handler` inside the save transaction for every batch of
    /// notifications (for example to maintain projections).
    pub fn with_event_handler(mut self, handler: D::Handler<N>) -> Self {
        self.handler = Some(handler);
        self
    }

    /// The configured retry policy.
    pub fn retry_config(&self) -> RetryConfig {
        self.retry
    }

    /// Builds the backend with default codecs.
    pub async fn build_default(self) -> Result<Backend<S, N>, BackendError>
    where
        D::Codec<S>: Default,
        D::Codec<N>: Default,
    {
        self.build(Default::default(), Default::default()).await
    }

    /// Builds the backend.
    pub async fn build(
        self,
        state_codec: D::Codec<S>,
        notification_codec: D::Codec<N>,
    ) -> Result<Backend<S, N>, BackendError> {
        let Storage {
            repository,
            outbox,
            updates,
        } = self
            .driver
            .build(
                Arc::clone(&self.model),
                state_codec,
                notification_codec,
                self.handler,
            )
            .await?;
        let repository: Arc<dyn super::Repository<S, N>> = match self.command_cache {
            CommandCacheChoice::Disabled => repository,
            CommandCacheChoice::InMemory(size) => Arc::new(CachedRepository::with_size(
                repository,
                Arc::new(InMemoryCommandStore::new(size)),
                self.state_cache_size,
            )),
            CommandCacheChoice::Custom(store) => Arc::new(CachedRepository::with_size(
                repository,
                store,
                self.state_cache_size,
            )),
        };
        let reader: Arc<dyn RepositoryReader<S>> = Arc::new(ReaderOf(Arc::clone(&repository)));
        Ok(Backend {
            handler: CommandHandler::with_retry(repository, self.retry),
            outbox,
            repository: reader,
            updates,
        })
    }
}

/// Exposes the read side of a repository.
struct ReaderOf<S, N>(Arc<dyn super::Repository<S, N>>);

#[async_trait::async_trait]
impl<S: Payload, N: Payload> RepositoryReader<S> for ReaderOf<S, N> {
    async fn get(&self, id: &str) -> Result<super::AggregateState<S>, BackendError> {
        self.0.get(id).await
    }
}
