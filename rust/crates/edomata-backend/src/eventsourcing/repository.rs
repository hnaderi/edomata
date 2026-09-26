//! The write side of an event-sourced storage.

use async_trait::async_trait;
use edomata_core::NonEmpty;

use super::CommandState;
use crate::{BackendError, CommandRef, SeqNr};

/// Loads aggregates for commands and appends the outcome of handling them.
///
/// `append` must be atomic: events, notifications and the command record
/// are written in one transaction, and a concurrent write to the same
/// aggregate (or a duplicate command id) fails with
/// [`BackendError::VersionConflict`].
#[async_trait]
pub trait Repository<S, E, R, N>: Send + Sync {
    /// Loads the aggregate targeted by `cmd`, or reports the command as
    /// redundant when it was already handled.
    async fn load(&self, cmd: CommandRef<'_>) -> Result<CommandState<S, E, R>, BackendError>;

    /// Appends the accepted events of a command, publishes its
    /// notifications and records the command, atomically.
    ///
    /// `version` is the version the aggregate had when it was loaded; the
    /// first appended event gets that version.
    async fn append(
        &self,
        cmd: CommandRef<'_>,
        version: SeqNr,
        new_state: S,
        events: NonEmpty<E>,
        notifications: Vec<N>,
    ) -> Result<(), BackendError>;

    /// Publishes notifications without touching the aggregate.
    async fn notify(
        &self,
        cmd: CommandRef<'_>,
        notifications: NonEmpty<N>,
    ) -> Result<(), BackendError>;
}
