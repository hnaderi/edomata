//! DDL generation for migration tools.

use crate::PGNaming;

/// Default payload column type.
pub const DEFAULT_PAYLOAD_TYPE: &str = "jsonb";

/// Generates plain SQL DDL for the Edomata tables, for use with migration
/// tools (Flyway, Liquibase...) instead of the drivers' automatic setup.
///
/// The output is byte-for-byte identical to Scala's `PGSchema`; the golden
/// tests under `rust/tests/golden/` assert it. Every payload column defaults
/// to `jsonb`. Payload type parameters are spliced into the DDL as is
/// (`"json"`, `"jsonb"` or `"bytea"` are the intended values).
///
/// ```
/// use edomata_postgres::{PGNaming, PGSchema};
///
/// let ddl = PGSchema::cqrs(&PGNaming::prefixed_str("auth").unwrap());
/// assert_eq!(ddl.len(), 3); // states, outbox, commands
/// assert!(ddl.iter().all(|s| s.ends_with(';')));
/// ```
#[derive(Clone, Copy, Debug, Default)]
pub struct PGSchema;

impl PGSchema {
    /// DDL for event-sourcing tables (journal, outbox, commands, snapshots,
    /// migrations) with `jsonb` payloads.
    pub fn eventsourcing(naming: &PGNaming) -> Vec<String> {
        Self::eventsourcing_with(
            naming,
            DEFAULT_PAYLOAD_TYPE,
            DEFAULT_PAYLOAD_TYPE,
            DEFAULT_PAYLOAD_TYPE,
        )
    }

    /// DDL for event-sourcing tables with explicit payload column types.
    pub fn eventsourcing_with(
        naming: &PGNaming,
        event_type: &str,
        notification_type: &str,
        snapshot_type: &str,
    ) -> Vec<String> {
        let mut out = ddl::schema_statement(naming);
        out.extend(ddl::journal_statements(naming, event_type));
        out.extend(ddl::outbox_statements(naming, notification_type));
        out.extend(ddl::commands_statements(naming));
        out.extend(ddl::snapshots_statements(naming, snapshot_type));
        out.extend(ddl::migrations_statements(naming));
        out
    }

    /// DDL for the optional `relay_checkpoints` table used by
    /// `edomata-broker`'s `JournalRelay`. Not included in
    /// [`eventsourcing`](Self::eventsourcing) / [`cqrs`](Self::cqrs), whose
    /// output stays byte-identical to Scala's; append it when journal
    /// streaming is enabled.
    pub fn relay_checkpoints(naming: &PGNaming) -> Vec<String> {
        ddl::relay_checkpoints_statements(naming)
    }

    /// DDL for CQRS tables (states, outbox, commands) with `jsonb` payloads.
    pub fn cqrs(naming: &PGNaming) -> Vec<String> {
        Self::cqrs_with(naming, DEFAULT_PAYLOAD_TYPE, DEFAULT_PAYLOAD_TYPE)
    }

    /// DDL for CQRS tables with explicit payload column types.
    pub fn cqrs_with(naming: &PGNaming, state_type: &str, notification_type: &str) -> Vec<String> {
        let mut out = ddl::schema_statement(naming);
        out.extend(ddl::states_statements(naming, state_type));
        out.extend(ddl::outbox_statements(naming, notification_type));
        out.extend(ddl::commands_statements(naming));
        out
    }
}

/// The individual DDL statements. Drivers reuse them for automatic setup so
/// that the tables they create match the generated migrations exactly.
pub mod ddl {
    use crate::PGNaming;

    /// `CREATE SCHEMA` in schema mode, nothing in prefix mode.
    pub fn schema_statement(naming: &PGNaming) -> Vec<String> {
        if naming.needs_schema_setup() {
            vec![format!(
                "CREATE SCHEMA IF NOT EXISTS \"{}\";",
                naming.namespace()
            )]
        } else {
            Vec::new()
        }
    }

    /// The journal table and its indexes.
    pub fn journal_statements(naming: &PGNaming, payload_type: &str) -> Vec<String> {
        let t = naming.table("journal");
        let pk = naming.constraint("journal_pk");
        let un = naming.constraint("journal_un");
        let seqnr_idx = naming.index("journal_seqnr_idx");
        let stream_idx = naming.index("journal_stream_idx");
        vec![
            format!(
                "CREATE TABLE IF NOT EXISTS {t} (\n  id uuid NOT NULL,\n  \"time\" timestamptz NOT NULL,\n  seqnr bigserial NOT NULL,\n  \"version\" int8 NOT NULL,\n  stream text NOT NULL,\n  payload {payload_type} NOT NULL,\n  CONSTRAINT {pk} PRIMARY KEY (id),\n  CONSTRAINT {un} UNIQUE (stream, version)\n);"
            ),
            format!("CREATE INDEX IF NOT EXISTS {seqnr_idx} ON {t} USING btree (seqnr);"),
            format!(
                "CREATE INDEX IF NOT EXISTS {stream_idx} ON {t} USING btree (stream, version);"
            ),
        ]
    }

    /// The outbox table.
    pub fn outbox_statements(naming: &PGNaming, payload_type: &str) -> Vec<String> {
        let t = naming.table("outbox");
        let pk = naming.constraint("outbox_pk");
        vec![format!(
            "CREATE TABLE IF NOT EXISTS {t} (\n  seqnr bigserial NOT NULL,\n  stream text NOT NULL,\n  correlation text NULL,\n  causation text NULL,\n  payload {payload_type} NOT NULL,\n  created timestamptz NOT NULL,\n  published timestamptz NULL,\n  CONSTRAINT {pk} PRIMARY KEY (seqnr)\n);"
        )]
    }

    /// The commands table.
    pub fn commands_statements(naming: &PGNaming) -> Vec<String> {
        let t = naming.table("commands");
        let pk = naming.constraint("commands_pk");
        vec![format!(
            "CREATE TABLE IF NOT EXISTS {t} (\n  id text NOT NULL,\n  \"time\" timestamptz NOT NULL,\n  address text NOT NULL,\n  CONSTRAINT {pk} PRIMARY KEY (id)\n);"
        )]
    }

    /// The snapshots table.
    pub fn snapshots_statements(naming: &PGNaming, payload_type: &str) -> Vec<String> {
        let t = naming.table("snapshots");
        let pk = naming.constraint("snapshots_pk");
        vec![format!(
            "CREATE TABLE IF NOT EXISTS {t} (\n  id text NOT NULL,\n  \"version\" int8 NOT NULL,\n  state {payload_type} NOT NULL,\n  CONSTRAINT {pk} PRIMARY KEY (id)\n);"
        )]
    }

    /// The migrations tracking table.
    pub fn migrations_statements(naming: &PGNaming) -> Vec<String> {
        let t = naming.table("migrations");
        let pk = naming.constraint("migrations_pk");
        vec![format!(
            "CREATE TABLE IF NOT EXISTS {t} (\n  \"version\" text NOT NULL,\n  description text NOT NULL,\n  applied_at timestamptz NOT NULL DEFAULT now(),\n  CONSTRAINT {pk} PRIMARY KEY (\"version\")\n);"
        )]
    }

    /// The relay checkpoints table of `edomata-broker`'s `JournalRelay`
    /// (opt-in: not part of [`super::PGSchema::eventsourcing`], which stays
    /// identical to Scala's output).
    pub fn relay_checkpoints_statements(naming: &PGNaming) -> Vec<String> {
        let t = naming.table("relay_checkpoints");
        let pk = naming.constraint("relay_checkpoints_pk");
        vec![format!(
            "CREATE TABLE IF NOT EXISTS {t} (\n  relay text NOT NULL,\n  seqnr int8 NOT NULL,\n  updated_at timestamptz NOT NULL DEFAULT now(),\n  CONSTRAINT {pk} PRIMARY KEY (relay)\n);"
        )]
    }

    /// The CQRS states table.
    pub fn states_statements(naming: &PGNaming, payload_type: &str) -> Vec<String> {
        let t = naming.table("states");
        let pk = naming.constraint("states_pk");
        vec![format!(
            "CREATE TABLE IF NOT EXISTS {t} (\n  id text NOT NULL,\n  \"version\" int8 NOT NULL,\n  state {payload_type} NOT NULL,\n  CONSTRAINT {pk} PRIMARY KEY (id)\n);"
        )]
    }
}
