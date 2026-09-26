//! Read and write sides of a CQRS storage.

use async_trait::async_trait;
use edomata_core::NonEmpty;

use super::{AggregateState, CommandState};
use crate::{BackendError, CommandRef, SeqNr};

/// Reads the current state of aggregates.
#[async_trait]
pub trait RepositoryReader<S>: Send + Sync {
    /// Current state of the aggregate; the model's initial state at version
    /// 0 when it was never saved.
    async fn get(&self, id: &str) -> Result<AggregateState<S>, BackendError>;
}

/// Loads aggregates for commands and saves the outcome of handling them.
///
/// `save` must be atomic: the state, the notifications and the command
/// record are written in one transaction, and a concurrent save of the same
/// aggregate (or a duplicate command id) fails with
/// [`BackendError::VersionConflict`].
#[async_trait]
pub trait Repository<S, N>: RepositoryReader<S> {
    /// Loads the aggregate targeted by `cmd`, or reports the command as
    /// redundant when it was already handled.
    async fn load(&self, cmd: CommandRef<'_>) -> Result<CommandState<S>, BackendError>;

    /// Saves the new state (expected to replace version `version`),
    /// publishes the notifications and records the command, atomically.
    async fn save(
        &self,
        cmd: CommandRef<'_>,
        version: SeqNr,
        new_state: S,
        notifications: Vec<N>,
    ) -> Result<(), BackendError>;

    /// Publishes notifications without touching the aggregate.
    async fn notify(
        &self,
        cmd: CommandRef<'_>,
        notifications: NonEmpty<N>,
    ) -> Result<(), BackendError>;
}
