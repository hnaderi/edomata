//! The event-sourcing backend and its builder, the counterparts of
//! `JBackend` and `JBackendBuilder`.

use std::sync::Arc;

use edomata_backend::eventsourcing::{Backend, JournalReader};
use edomata_backend::{BackendError, EventMessage, OutboxItem, OutboxReader, Payload, RetryConfig};
use edomata_core::{
    BoxFuture, CommandMessage, DomainDsl, Edomaton, NonEmpty, RequestContext, ResponseD,
};
use edomata_postgres::PGNaming;
use edomata_sqlx::{PgPool, SqlxCodec, SqlxDriver};
use futures::TryStreamExt;
use serde::Serialize;
use serde::de::DeserializeOwned;
use sqlx::postgres::PgPoolOptions;

use crate::runtime::{BlockingBackend, SimpleRuntime};
use crate::{CommandHandler, Context, ModelAdapter, SimpleCodec, SimpleDomainModel, SimpleError};

/// Outcome of handling a command: `Ok(Ok(()))` when accepted or already
/// handled, `Ok(Err(reasons))` when rejected, `Err` on storage failure.
pub type HandleResult<R> = Result<Result<(), Vec<R>>, SimpleError>;

/// The backend built for a [`SimpleDomainModel`] `M` and notifications `N`.
pub type BuiltBackend<M, N> = SimpleBackend<
    <M as SimpleDomainModel>::State,
    <M as SimpleDomainModel>::Event,
    <M as SimpleDomainModel>::Rejection,
    N,
>;

/// The blocking backend built for a [`SimpleDomainModel`] `M` and
/// notifications `N`.
pub type BuiltBlockingBackend<M, N> = BlockingBackend<
    <M as SimpleDomainModel>::State,
    <M as SimpleDomainModel>::Event,
    <M as SimpleDomainModel>::Rejection,
    N,
>;

/// A compiled handler: a function from a command message to a
/// [`HandleResult`].
pub type SimpleService<C, R> =
    Arc<dyn Fn(CommandMessage<C>) -> BoxFuture<'static, HandleResult<R>> + Send + Sync>;

/// An event-sourced backend on PostgreSQL with a closure-based API.
/// Mirrors Scala's `JBackend`; build one with [`SimpleBackend::builder`].
pub struct SimpleBackend<S, E, R, N> {
    inner: Backend<S, E, R, N>,
}

impl<S, E, R, N> Clone for SimpleBackend<S, E, R, N> {
    fn clone(&self) -> Self {
        Self {
            inner: self.inner.clone(),
        }
    }
}

impl<S, E, R, N> std::fmt::Debug for SimpleBackend<S, E, R, N> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("SimpleBackend")
    }
}

impl<S: Payload, E: Payload, R: Payload, N: Payload> SimpleBackend<S, E, R, N> {
    /// Starts building a backend for `model` (`JBackendBuilder.forDoobie`).
    pub fn builder<M>(model: M) -> SimpleBackendBuilder<M, N>
    where
        M: SimpleDomainModel<State = S, Event = E, Rejection = R>,
    {
        SimpleBackendBuilder {
            model,
            naming: None,
            pool: None,
            database_url: None,
            event_codec: None,
            notification_codec: None,
            max_retry: 5,
            in_mem_snapshot_size: 1000,
            skip_setup: false,
        }
    }

    /// Wraps an already built core backend.
    pub fn from_backend(inner: Backend<S, E, R, N>) -> Self {
        Self { inner }
    }

    /// The core backend.
    pub fn inner(&self) -> &Backend<S, E, R, N> {
        &self.inner
    }

    /// Compiles a handler into a reusable service.
    pub fn compile<C: Payload>(
        &self,
        handler: &CommandHandler<C, S, E, R, N>,
    ) -> SimpleService<C, R> {
        let handler = handler.clone();
        let app: Edomaton<RequestContext<C, S>, R, E, N, ()> =
            Edomaton::new(move |ctx: RequestContext<C, S>| {
                let run = handler.call(Context::from_request(ctx));
                async move {
                    let out = run.await;
                    // As in Scala, a rejection without reasons is a programming
                    // error (`IllegalArgumentException`).
                    let decision = out
                        .decision
                        .into_decision()
                        .expect("Rejected must have at least one reason");
                    ResponseD::new(decision, out.notifications)
                }
            });
        let service = self.inner.compile(app);
        Arc::new(move |cmd| {
            let fut = service(cmd);
            Box::pin(async move {
                match fut.await {
                    Ok(Ok(())) => Ok(Ok(())),
                    Ok(Err(reasons)) => Ok(Err(reasons.into_vec())),
                    Err(e) => Err(SimpleError::Backend(e)),
                }
            })
        })
    }

    /// Handles one command with `handler` (`JBackend.handle`).
    pub async fn handle<C: Payload>(
        &self,
        handler: &CommandHandler<C, S, E, R, N>,
        command: CommandMessage<C>,
    ) -> HandleResult<R> {
        self.compile(handler)(command).await
    }

    /// The journal (`JBackend.journal`).
    pub fn journal(&self) -> SimpleJournal<E> {
        SimpleJournal {
            inner: Arc::clone(self.inner.journal()),
        }
    }

    /// The outbox (`JBackend.outbox`).
    pub fn outbox(&self) -> SimpleOutbox<N> {
        SimpleOutbox {
            inner: Arc::clone(self.inner.outbox()),
        }
    }

    /// Releases the backend's resources (`JBackend.close`).
    pub async fn close(&self) -> Result<(), SimpleError> {
        self.inner.close().await.map_err(SimpleError::Backend)
    }
}

/// Journal access returning vectors instead of streams (`JJournalReader`).
pub struct SimpleJournal<E> {
    inner: Arc<dyn JournalReader<E>>,
}

impl<E> Clone for SimpleJournal<E> {
    fn clone(&self) -> Self {
        Self {
            inner: Arc::clone(&self.inner),
        }
    }
}

impl<E> std::fmt::Debug for SimpleJournal<E> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("SimpleJournal")
    }
}

impl<E: Payload> SimpleJournal<E> {
    /// All events of a stream (aggregate).
    pub async fn read_stream(&self, stream_id: &str) -> Result<Vec<EventMessage<E>>, SimpleError> {
        collect(self.inner.read_stream(stream_id)).await
    }

    /// The events of a stream after a version.
    pub async fn read_stream_after(
        &self,
        stream_id: &str,
        version: i64,
    ) -> Result<Vec<EventMessage<E>>, SimpleError> {
        collect(self.inner.read_stream_after(stream_id, version)).await
    }

    /// All events across all streams.
    pub async fn read_all(&self) -> Result<Vec<EventMessage<E>>, SimpleError> {
        collect(self.inner.read_all()).await
    }

    /// All events across all streams after a sequence number.
    pub async fn read_all_after(&self, seq_nr: i64) -> Result<Vec<EventMessage<E>>, SimpleError> {
        collect(self.inner.read_all_after(seq_nr)).await
    }

    /// The underlying reader.
    pub fn inner(&self) -> &Arc<dyn JournalReader<E>> {
        &self.inner
    }
}

/// Outbox access returning vectors instead of streams (`JOutboxReader`,
/// plus the `mark_*` operations Java lacked).
pub struct SimpleOutbox<N> {
    inner: Arc<dyn OutboxReader<N>>,
}

impl<N> Clone for SimpleOutbox<N> {
    fn clone(&self) -> Self {
        Self {
            inner: Arc::clone(&self.inner),
        }
    }
}

impl<N> std::fmt::Debug for SimpleOutbox<N> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("SimpleOutbox")
    }
}

impl<N: Payload> SimpleOutbox<N> {
    /// All pending items.
    pub async fn read(&self) -> Result<Vec<OutboxItem<N>>, SimpleError> {
        collect(self.inner.read()).await
    }

    /// Marks one item as sent.
    pub async fn mark_as_sent(&self, item: &OutboxItem<N>) -> Result<(), SimpleError> {
        self.inner
            .mark_as_sent(item)
            .await
            .map_err(SimpleError::Backend)
    }

    /// Marks items as sent (a no-op for an empty slice).
    pub async fn mark_all_as_sent(&self, items: &[OutboxItem<N>]) -> Result<(), SimpleError> {
        match NonEmpty::from_vec(items.to_vec()) {
            Some(items) => self
                .inner
                .mark_all_as_sent(&items)
                .await
                .map_err(SimpleError::Backend),
            None => Ok(()),
        }
    }

    /// The underlying reader.
    pub fn inner(&self) -> &Arc<dyn OutboxReader<N>> {
        &self.inner
    }
}

async fn collect<T>(
    stream: futures::stream::BoxStream<'_, Result<T, BackendError>>,
) -> Result<Vec<T>, SimpleError> {
    stream.try_collect().await.map_err(SimpleError::Backend)
}

/// Builder of a [`SimpleBackend`] (`JBackendBuilder`). Required settings:
/// a namespace, a connection ([`pool`](Self::pool) or
/// [`database_url`](Self::database_url)) and both codecs.
pub struct SimpleBackendBuilder<M: SimpleDomainModel, N> {
    model: M,
    naming: Option<Result<PGNaming, SimpleError>>,
    pool: Option<PgPool>,
    database_url: Option<String>,
    event_codec: Option<SqlxCodec<M::Event>>,
    notification_codec: Option<SqlxCodec<N>>,
    max_retry: u32,
    in_mem_snapshot_size: usize,
    skip_setup: bool,
}

impl<M: SimpleDomainModel, N> std::fmt::Debug for SimpleBackendBuilder<M, N> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SimpleBackendBuilder")
            .field(
                "naming",
                &self
                    .naming
                    .as_ref()
                    .map(|n| n.as_ref().map(|n| format!("{n:?}"))),
            )
            .field("has_pool", &self.pool.is_some())
            .field(
                "database_url",
                &self.database_url.as_deref().map(|_| "<set>"),
            )
            .field("has_event_codec", &self.event_codec.is_some())
            .field("has_notification_codec", &self.notification_codec.is_some())
            .field("max_retry", &self.max_retry)
            .field("in_mem_snapshot_size", &self.in_mem_snapshot_size)
            .field("skip_setup", &self.skip_setup)
            .finish()
    }
}

impl<M, N> SimpleBackendBuilder<M, N>
where
    M: SimpleDomainModel,
    M::State: Payload,
    M::Event: Payload,
    M::Rejection: Payload,
    N: Payload,
{
    /// The PostgreSQL namespace, with the prefixed naming strategy
    /// (`ns_journal`, ...). An invalid name is reported by `build`.
    pub fn namespace(mut self, ns: &str) -> Self {
        self.naming = Some(crate::schema::prefixed(ns));
        self
    }

    /// The PostgreSQL namespace, with the schema naming strategy
    /// (`"ns".journal`, ...). An invalid name is reported by `build`.
    pub fn schema_namespace(mut self, ns: &str) -> Self {
        self.naming = Some(crate::schema::schema(ns));
        self
    }

    /// An explicit naming strategy.
    pub fn naming(mut self, naming: PGNaming) -> Self {
        self.naming = Some(Ok(naming));
        self
    }

    /// The connection pool (`JBackendBuilder.dataSource`).
    pub fn pool(mut self, pool: PgPool) -> Self {
        self.pool = Some(pool);
        self
    }

    /// A connection URL; a pool is opened by `build` when no pool is set.
    pub fn database_url(mut self, url: &str) -> Self {
        self.database_url = Some(url.to_string());
        self
    }

    /// The event codec (from [`SimpleCodec::into_codec`] or
    /// [`serde_codec`](crate::serde_codec)).
    pub fn event_codec(mut self, codec: SqlxCodec<M::Event>) -> Self {
        self.event_codec = Some(codec);
        self
    }

    /// The notification codec.
    pub fn notification_codec(mut self, codec: SqlxCodec<N>) -> Self {
        self.notification_codec = Some(codec);
        self
    }

    /// The event codec from a [`SimpleCodec`].
    pub fn simple_event_codec(self, codec: impl SimpleCodec<M::Event>) -> Self {
        self.event_codec(codec.into_codec())
    }

    /// The notification codec from a [`SimpleCodec`].
    pub fn simple_notification_codec(self, codec: impl SimpleCodec<N>) -> Self {
        self.notification_codec(codec.into_codec())
    }

    /// Both codecs as serde `jsonb` codecs.
    pub fn serde_codecs(self) -> Self
    where
        M::Event: Serialize + DeserializeOwned,
        N: Serialize + DeserializeOwned,
    {
        self.event_codec(SqlxCodec::jsonb())
            .notification_codec(SqlxCodec::jsonb())
    }

    /// Maximum number of retries on version conflicts (default 5).
    pub fn max_retry(mut self, n: u32) -> Self {
        self.max_retry = n;
        self
    }

    /// Size of the in-memory snapshot cache (default 1000).
    pub fn in_mem_snapshot_size(mut self, size: usize) -> Self {
        self.in_mem_snapshot_size = size;
        self
    }

    /// Skips the automatic DDL setup (for Flyway / manual migrations).
    pub fn skip_setup(mut self, skip: bool) -> Self {
        self.skip_setup = skip;
        self
    }

    /// Builds the backend, opening the connection pool when needed and
    /// creating the tables unless `skip_setup` is set.
    pub async fn build(self) -> Result<BuiltBackend<M, N>, SimpleError> {
        let naming = self
            .naming
            .ok_or(SimpleError::MissingConfig("namespace"))??;
        let event_codec = self
            .event_codec
            .ok_or(SimpleError::MissingConfig("eventCodec"))?;
        let notification_codec = self
            .notification_codec
            .ok_or(SimpleError::MissingConfig("notificationCodec"))?;
        let pool = match (self.pool, self.database_url) {
            (Some(pool), _) => pool,
            (None, Some(url)) => PgPoolOptions::new()
                .connect(&url)
                .await
                .map_err(SimpleError::Connection)?,
            (None, None) => return Err(SimpleError::MissingConfig("pool or databaseUrl")),
        };
        let driver = SqlxDriver::new_with(naming, pool, self.skip_setup).await?;
        let inner = Backend::builder(ModelAdapter(self.model), DomainDsl::<(), _, _, _, N>::new())
            .driver(driver)
            .in_mem_snapshot(self.in_mem_snapshot_size)
            .with_retry_config(RetryConfig {
                max_retry: self.max_retry,
                ..RetryConfig::default()
            })
            .build(event_codec, notification_codec)
            .await?;
        Ok(SimpleBackend { inner })
    }

    /// Builds the backend on `runtime` and wraps it for blocking use
    /// (`JBackendBuilder.build(runtime)`).
    pub fn build_blocking(
        self,
        runtime: SimpleRuntime,
    ) -> Result<BuiltBlockingBackend<M, N>, SimpleError> {
        let backend = runtime.block_on(self.build())?;
        Ok(BlockingBackend::new(backend, runtime))
    }
}
