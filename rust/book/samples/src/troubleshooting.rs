//! Samples of the "Troubleshooting" chapter.

use edomata_backend::{BackendError, CommandResult};

// ANCHOR: classify
/// How an application maps the outcome of a command, for instance to HTTP
/// status codes.
pub fn status_code<R: std::fmt::Debug>(result: &CommandResult<R>) -> u16 {
    match result {
        // Accepted, indecisive, or a redundant (already handled) command.
        Ok(Ok(())) => 200,
        // A business rejection: expected, not an error.
        Ok(Err(_reasons)) => 422,
        // Concurrent writers kept winning for every attempt of the retry
        // policy: retrying later with the same command id is safe.
        Err(BackendError::MaxRetryExceeded) => 409,
        // Only raised by direct storage calls: compiled services retry it
        // and report `MaxRetryExceeded` instead.
        Err(BackendError::VersionConflict) => 409,
        // A codec or invariant failure: needs a fix, not a retry.
        Err(BackendError::PersistenceError(_)) => 500,
        // A driver error (connection, SQL...), wrapped with its source.
        Err(BackendError::UnknownError(_)) => 503,
    }
}

/// The `sqlx` error behind an `UnknownError`, for logs and alerts.
pub fn sqlx_cause(error: &BackendError) -> Option<&sqlx::Error> {
    match error {
        BackendError::UnknownError(source) => source.downcast_ref::<sqlx::Error>(),
        _ => None,
    }
}
// ANCHOR_END: classify

#[cfg(test)]
mod tests {
    use super::*;
    use crate::eventsourcing::Event;

    #[test]
    fn outcomes_map_to_status_codes() {
        assert_eq!(status_code::<()>(&Ok(Ok(()))), 200);
        assert_eq!(status_code::<()>(&Err(BackendError::MaxRetryExceeded)), 409);
        let wrapped = BackendError::unknown(sqlx::Error::PoolTimedOut);
        assert!(matches!(
            sqlx_cause(&wrapped),
            Some(sqlx::Error::PoolTimedOut)
        ));
        assert_eq!(status_code::<()>(&Err(wrapped)), 503);
    }

    #[test]
    fn codec_errors_become_persistence_errors() {
        // ANCHOR: codec_error
        use edomata_backend::{Codec, CodecError};
        use edomata_serde::SerdeCodec;

        // A payload written by another version of the code (or by Scala with
        // another JSON shape) that `Event` cannot read.
        let codec = SerdeCodec::<Event>::jsonb();
        let error = codec.decode(br#"{"Opened":{}}"#).unwrap_err();
        assert!(matches!(error, CodecError::Decode(_)));
        // Reading the journal reports it as a `PersistenceError`.
        let backend_error = BackendError::from(error);
        assert!(matches!(
            backend_error,
            BackendError::PersistenceError(ref message) if message.starts_with("decoding failed")
        ));
        // `Opened` is a unit variant, serialized as a bare string.
        assert_eq!(codec.encode(&Event::Opened).unwrap(), br#""Opened""#);
        // ANCHOR_END: codec_error
    }

    #[test]
    fn invalid_namespaces_are_rejected_before_any_sql() {
        // ANCHOR: namespace_error
        use edomata_sqlx::{PGNamespace, PGNaming};

        // PostgreSQL identifiers: a letter or `_`, then letters, digits, `_`
        // or `$`, at most 63 characters.
        assert!(PGNaming::prefixed_str("order-service").is_err());
        assert!(PGNaming::schema_str("1accounts").is_err());
        assert!(PGNamespace::from_string(&"a".repeat(64)).is_err());
        assert!(PGNaming::prefixed_str("order_service").is_ok());
        // ANCHOR_END: namespace_error
    }
}
