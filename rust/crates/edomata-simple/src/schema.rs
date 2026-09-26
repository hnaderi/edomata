//! DDL helper taking namespaces as strings, the counterpart of `JPGSchema`.

use edomata_postgres::{PGNamespace, PGNaming, PGSchema};

use crate::SimpleError;

/// DDL generation for migration tools (Flyway, Liquibase, ...), taking the
/// namespace as a string. Mirrors Scala's `JPGSchema`: the plain methods
/// use the prefixed naming strategy (`ns_journal`, ...), the `*_with_schema`
/// ones create a PostgreSQL schema (`"ns".journal`). An invalid namespace
/// is reported as [`SimpleError::InvalidNamespace`].
#[derive(Clone, Copy, Debug, Default)]
pub struct SimplePGSchema;

impl SimplePGSchema {
    /// Event sourcing tables (journal, outbox, commands, snapshots,
    /// migrations) with `jsonb` payloads.
    pub fn eventsourcing(namespace: &str) -> Result<Vec<String>, SimpleError> {
        Ok(PGSchema::eventsourcing(&prefixed(namespace)?))
    }

    /// Event sourcing tables with explicit payload types (`"json"`,
    /// `"jsonb"` or `"bytea"`).
    pub fn eventsourcing_with(
        namespace: &str,
        event_type: &str,
        notification_type: &str,
        snapshot_type: &str,
    ) -> Result<Vec<String>, SimpleError> {
        Ok(PGSchema::eventsourcing_with(
            &prefixed(namespace)?,
            event_type,
            notification_type,
            snapshot_type,
        ))
    }

    /// CQRS tables (states, outbox, commands) with `jsonb` payloads.
    pub fn cqrs(namespace: &str) -> Result<Vec<String>, SimpleError> {
        Ok(PGSchema::cqrs(&prefixed(namespace)?))
    }

    /// CQRS tables with explicit payload types.
    pub fn cqrs_with(
        namespace: &str,
        state_type: &str,
        notification_type: &str,
    ) -> Result<Vec<String>, SimpleError> {
        Ok(PGSchema::cqrs_with(
            &prefixed(namespace)?,
            state_type,
            notification_type,
        ))
    }

    /// Event sourcing tables in a dedicated PostgreSQL schema.
    pub fn eventsourcing_with_schema(namespace: &str) -> Result<Vec<String>, SimpleError> {
        Ok(PGSchema::eventsourcing(&schema(namespace)?))
    }

    /// CQRS tables in a dedicated PostgreSQL schema.
    pub fn cqrs_with_schema(namespace: &str) -> Result<Vec<String>, SimpleError> {
        Ok(PGSchema::cqrs(&schema(namespace)?))
    }
}

pub(crate) fn namespace(ns: &str) -> Result<PGNamespace, SimpleError> {
    PGNamespace::from_string(ns).map_err(|e| SimpleError::InvalidNamespace(e.to_string()))
}

pub(crate) fn prefixed(ns: &str) -> Result<PGNaming, SimpleError> {
    Ok(PGNaming::prefixed(namespace(ns)?))
}

pub(crate) fn schema(ns: &str) -> Result<PGNaming, SimpleError> {
    Ok(PGNaming::schema(namespace(ns)?))
}
