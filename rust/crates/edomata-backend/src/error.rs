//! Backend errors.

use std::fmt;

/// Errors raised by backends. Mirrors Scala's `BackendError`.
///
/// `Redundant` is not an error: it belongs to the command state (see
/// [`eventsourcing::CommandState`](crate::eventsourcing::CommandState)).
#[derive(Debug, thiserror::Error)]
pub enum BackendError {
    /// Another writer changed the aggregate since it was loaded.
    #[error("You can't proceed due to version conflict, read and decide again!")]
    VersionConflict,
    /// Retrying on version conflicts did not succeed within the allowed
    /// number of attempts.
    #[error("Maximum number of retries exceeded!")]
    MaxRetryExceeded,
    /// The storage did not behave as expected.
    #[error("{0}")]
    PersistenceError(String),
    /// Any other error, wrapped.
    #[error("Unknown error!")]
    UnknownError(#[source] Box<dyn std::error::Error + Send + Sync + 'static>),
}

impl BackendError {
    /// Wraps any error as [`BackendError::UnknownError`].
    pub fn unknown<E: std::error::Error + Send + Sync + 'static>(error: E) -> Self {
        BackendError::UnknownError(Box::new(error))
    }

    /// Builds a [`BackendError::PersistenceError`].
    pub fn persistence(message: impl fmt::Display) -> Self {
        BackendError::PersistenceError(message.to_string())
    }

    /// Whether this is a [`BackendError::VersionConflict`].
    pub fn is_version_conflict(&self) -> bool {
        matches!(self, BackendError::VersionConflict)
    }
}

impl PartialEq for BackendError {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (BackendError::VersionConflict, BackendError::VersionConflict)
            | (BackendError::MaxRetryExceeded, BackendError::MaxRetryExceeded) => true,
            (BackendError::PersistenceError(a), BackendError::PersistenceError(b)) => a == b,
            (BackendError::UnknownError(a), BackendError::UnknownError(b)) => {
                a.to_string() == b.to_string()
            }
            _ => false,
        }
    }
}

impl Eq for BackendError {}

impl From<crate::CodecError> for BackendError {
    fn from(error: crate::CodecError) -> Self {
        BackendError::PersistenceError(error.to_string())
    }
}
