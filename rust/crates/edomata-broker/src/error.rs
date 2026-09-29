//! Relay errors.

use edomata_backend::BackendError;

use crate::publisher::{BoxError, PublishError};

/// Why a relay pass or a running relay stopped.
///
/// Transient publish failures never surface here while the
/// [`RetryPolicy`](crate::RetryPolicy) budget lasts: they are retried. Every
/// variant means the current batch was **not** marked as sent (or
/// checkpointed), so it is published again by the next pass or the next
/// relay: delivery stays at-least-once.
#[derive(Debug, thiserror::Error)]
pub enum RelayError {
    /// Reading the outbox / journal or marking items failed.
    #[error(transparent)]
    Backend(#[from] BackendError),
    /// A payload could not be encoded.
    #[error("could not encode payload: {0}")]
    Encode(String),
    /// A permanent publish failure ([`PublishError::Permanent`]), or a
    /// transient one ([`PublishError::Transient`]) after the retry budget
    /// was exhausted.
    #[error(transparent)]
    Publish(#[from] PublishError),
    /// Loading or saving a journal checkpoint failed.
    #[error("checkpoint store failed: {0}")]
    Checkpoint(#[source] BoxError),
    /// Leader election failed (database error).
    #[error("leader election failed: {0}")]
    Leader(#[source] BoxError),
    /// The relay was cancelled while waiting to retry a batch. Cancellation
    /// between passes is not an error: `run` returns `Ok(())`.
    #[error("relay cancelled")]
    Cancelled,
}
