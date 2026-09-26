//! A CQRS repository decorated with command idempotency and state caches.

use std::sync::Arc;

use async_trait::async_trait;
use edomata_core::NonEmpty;

use super::{AggregateState, CommandState, Repository, RepositoryReader};
use crate::{
    BackendError, Cache, CommandRef, CommandStore, InMemoryCommandStore, LruCache, Payload, SeqNr,
    StreamId,
};

/// Wraps a [`Repository`] with a [`CommandStore`] (to short-circuit
/// redundant commands) and a state [`Cache`] (to skip loading cached
/// aggregates). Reads through [`RepositoryReader::get`] always hit the
/// underlying storage.
pub struct CachedRepository<S, N> {
    commands: Arc<dyn CommandStore>,
    cache: Arc<dyn Cache<StreamId, AggregateState<S>>>,
    underlying: Arc<dyn Repository<S, N>>,
}

impl<S: Payload, N: Payload> CachedRepository<S, N> {
    /// Decorates `underlying` with the given caches.
    pub fn new(
        underlying: Arc<dyn Repository<S, N>>,
        commands: Arc<dyn CommandStore>,
        cache: Arc<dyn Cache<StreamId, AggregateState<S>>>,
    ) -> Self {
        Self {
            commands,
            cache,
            underlying,
        }
    }

    /// Decorates `underlying` with a state cache of `size` entries.
    pub fn with_size(
        underlying: Arc<dyn Repository<S, N>>,
        commands: Arc<dyn CommandStore>,
        size: usize,
    ) -> Self {
        Self::new(underlying, commands, Arc::new(LruCache::new(size)))
    }

    /// Decorates `underlying` with in-memory caches of `size` states and
    /// `max_commands` command ids.
    pub fn build(underlying: Arc<dyn Repository<S, N>>, size: usize, max_commands: usize) -> Self {
        Self::with_size(
            underlying,
            Arc::new(InMemoryCommandStore::new(max_commands)),
            size,
        )
    }
}

#[async_trait]
impl<S: Payload, N: Payload> RepositoryReader<S> for CachedRepository<S, N> {
    async fn get(&self, id: &str) -> Result<AggregateState<S>, BackendError> {
        self.underlying.get(id).await
    }
}

#[async_trait]
impl<S: Payload, N: Payload> Repository<S, N> for CachedRepository<S, N> {
    async fn load(&self, cmd: CommandRef<'_>) -> Result<CommandState<S>, BackendError> {
        if self.commands.contains(cmd.id).await? {
            return Ok(CommandState::Redundant);
        }
        match self.cache.get(&cmd.address.to_owned()).await {
            Some(cached) => Ok(CommandState::Aggregate(cached)),
            None => self.underlying.load(cmd).await,
        }
    }

    async fn save(
        &self,
        cmd: CommandRef<'_>,
        version: SeqNr,
        new_state: S,
        notifications: Vec<N>,
    ) -> Result<(), BackendError> {
        let cached = AggregateState::new(new_state.clone(), version + 1);
        self.underlying
            .save(cmd, version, new_state, notifications)
            .await?;
        self.cache
            .replace(cmd.address.to_owned(), cached, &move |existing| {
                existing.version <= version
            })
            .await;
        self.commands.append(cmd.id).await
    }

    async fn notify(
        &self,
        cmd: CommandRef<'_>,
        notifications: NonEmpty<N>,
    ) -> Result<(), BackendError> {
        self.underlying.notify(cmd, notifications).await
    }
}
