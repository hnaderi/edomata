//! sqlx error mapping.

use edomata_backend::BackendError;

/// PostgreSQL SQLSTATE of `unique_violation`.
const UNIQUE_VIOLATION: &str = "23505";

/// Whether a sqlx error is a PostgreSQL unique-constraint violation.
pub fn is_unique_violation(error: &sqlx::Error) -> bool {
    match error {
        sqlx::Error::Database(db) => db.code().as_deref() == Some(UNIQUE_VIOLATION),
        _ => false,
    }
}

/// Maps a sqlx error raised while writing an aggregate: a unique violation
/// (duplicate `(stream, version)` or command id) is a version conflict, as
/// in the Scala drivers; anything else is wrapped.
pub fn map_write(error: sqlx::Error) -> BackendError {
    if is_unique_violation(&error) {
        BackendError::VersionConflict
    } else {
        map_sqlx(error)
    }
}

/// Wraps any sqlx error.
pub fn map_sqlx(error: sqlx::Error) -> BackendError {
    BackendError::unknown(error)
}

/// Checks that exactly `expected` rows were affected, like the Scala
/// `assertInserted` helper.
pub fn assert_inserted(affected: u64, expected: u64) -> Result<(), BackendError> {
    if affected == expected {
        Ok(())
    } else {
        Err(BackendError::persistence(format!(
            "expected to insert exactly {expected}, but inserted {affected}"
        )))
    }
}
