//! Relay errors.

use edomata_backend::BackendError;

use crate::publisher::{BoxError, PublishError};

/// Why a relay stopped.
#[derive(Debug, thiserror::Error)]
pub enum RelayError {
    /// Reading the outbox / journal or marking items failed.
    #[error(transparent)]
    Backend(#[from] BackendError),
    /// A payload could not be encoded.
    #[error("could not encode payload: {0}")]
    Encode(String),
    /// A permanent publish failure, or the retry budget was exhausted.
    #[error(transparent)]
    Publish(#[from] PublishError),
    /// Loading or saving a journal checkpoint failed.
    #[error("checkpoint store failed: {0}")]
    Checkpoint(#[source] BoxError),
    /// Leader election failed (database error).
    #[error("leader election failed: {0}")]
    Leader(#[source] BoxError),
    /// The relay was cancelled while retrying.
    #[error("relay cancelled")]
    Cancelled,
}
