//! A repository decorated with command idempotency and snapshot caches.

use std::sync::Arc;

use async_trait::async_trait;
use edomata_core::NonEmpty;

use super::{CommandState, Repository, SnapshotStore, ValidState};
use crate::{BackendError, CommandRef, CommandStore, Payload, SeqNr};

/// Wraps a [`Repository`] with a [`CommandStore`] (to short-circuit
/// redundant commands) and a [`SnapshotStore`] (to avoid folding the
/// journal when the aggregate is cached).
pub struct CachedRepository<S, E, R, N> {
    underlying: Arc<dyn Repository<S, E, R, N>>,
    commands: Arc<dyn CommandStore>,
    snapshot: Arc<dyn SnapshotStore<S>>,
}

impl<S, E, R, N> CachedRepository<S, E, R, N> {
    /// Decorates `underlying`.
    pub fn new(
        underlying: Arc<dyn Repository<S, E, R, N>>,
        commands: Arc<dyn CommandStore>,
        snapshot: Arc<dyn SnapshotStore<S>>,
    ) -> Self {
        Self {
            underlying,
            commands,
            snapshot,
        }
    }
}

#[async_trait]
impl<S: Payload, E: Payload, R: Payload, N: Payload> Repository<S, E, R, N>
    for CachedRepository<S, E, R, N>
{
    async fn load(&self, cmd: CommandRef<'_>) -> Result<CommandState<S, E, R>, BackendError> {
        if self.commands.contains(cmd.id).await? {
            return Ok(CommandState::Redundant);
        }
        match self.snapshot.get_fast(cmd.address).await {
            Some(cached) => Ok(CommandState::from(cached)),
            None => self.underlying.load(cmd).await,
        }
    }

    async fn append(
        &self,
        cmd: CommandRef<'_>,
        version: SeqNr,
        new_state: S,
        events: NonEmpty<E>,
        notifications: Vec<N>,
    ) -> Result<(), BackendError> {
        let new_version = version + events.len() as SeqNr;
        let snapshot = ValidState::new(new_state.clone(), new_version);
        self.underlying
            .append(cmd, version, new_state, events, notifications)
            .await?;
        self.commands.append(cmd.id).await?;
        self.snapshot.put(cmd.address, snapshot).await
    }

    async fn notify(
        &self,
        cmd: CommandRef<'_>,
        notifications: NonEmpty<N>,
    ) -> Result<(), BackendError> {
        self.underlying.notify(cmd, notifications).await
    }
}
