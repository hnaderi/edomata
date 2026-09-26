//! Errors of the simple facade.

use edomata_backend::BackendError;

/// Errors raised by the simple facade. Configuration errors replace the
/// `IllegalArgumentException` / `IllegalStateException` of the Java API;
/// storage errors wrap [`BackendError`].
#[derive(Debug, thiserror::Error)]
pub enum SimpleError {
    /// A namespace string is not a valid PostgreSQL identifier.
    #[error("Invalid namespace: {0}")]
    InvalidNamespace(String),
    /// A required builder setting is missing (`"namespace is required"`,
    /// ...).
    #[error("{0} is required")]
    MissingConfig(&'static str),
    /// Connecting to the database failed.
    #[error("could not connect to PostgreSQL: {0}")]
    Connection(#[source] sqlx::Error),
    /// A storage error.
    #[error(transparent)]
    Backend(#[from] BackendError),
}

impl SimpleError {
    /// Whether this is a storage error ([`SimpleError::Backend`]).
    pub fn is_backend(&self) -> bool {
        matches!(self, SimpleError::Backend(_))
    }
}
