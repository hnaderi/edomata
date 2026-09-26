#![allow(missing_docs)]
//! SQL statements, mirroring `Queries.scala` of the Skunk and Doobie
//! drivers: same tables, columns, constraints, index names and ordering.
//! Setup DDL comes from `edomata_postgres::ddl`, so the tables created by the
//! driver are exactly the ones `PGSchema` generates for migration tools.

use edomata_postgres::{PGNaming, ddl};

/// Journal table statements.
pub struct JournalQueries {
    pub setup: Vec<String>,
    pub insert: String,
    pub read_all: String,
    pub read_all_after: String,
    pub read_all_before: String,
    pub read_stream: String,
    pub read_stream_after: String,
    pub read_stream_before: String,
}

const JOURNAL_FIELDS: &str = "id, time, seqnr, version, stream, payload";

impl JournalQueries {
    pub fn new(naming: &PGNaming, payload_type: &str) -> Self {
        let t = naming.table("journal");
        Self {
            setup: ddl::journal_statements(naming, payload_type),
            insert: format!(
                "insert into {t} (\"id\", \"stream\", \"time\", \"version\", \"payload\") values ($1, $2, $3, $4, $5)"
            ),
            read_all: format!("select {JOURNAL_FIELDS} from {t} order by seqnr asc"),
            read_all_after: format!(
                "select {JOURNAL_FIELDS} from {t} where seqnr > $1 order by seqnr asc"
            ),
            read_all_before: format!(
                "select {JOURNAL_FIELDS} from {t} where seqnr < $1 order by seqnr asc"
            ),
            read_stream: format!(
                "select {JOURNAL_FIELDS} from {t} where stream = $1 order by version asc"
            ),
            read_stream_after: format!(
                "select {JOURNAL_FIELDS} from {t} where stream = $1 and version > $2 order by version asc"
            ),
            read_stream_before: format!(
                "select {JOURNAL_FIELDS} from {t} where stream = $1 and version < $2 order by version asc"
            ),
        }
    }
}

/// Outbox table statements.
pub struct OutboxQueries {
    pub setup: Vec<String>,
    pub insert: String,
    pub read: String,
    pub mark_published: String,
}

impl OutboxQueries {
    pub fn new(naming: &PGNaming, payload_type: &str) -> Self {
        let t = naming.table("outbox");
        Self {
            setup: ddl::outbox_statements(naming, payload_type),
            insert: format!(
                "insert into {t} (payload, stream, created, correlation, causation) values ($1, $2, $3, $4, $5)"
            ),
            read: format!(
                "select seqnr, stream, created, payload, correlation, causation from {t} where published is NULL order by seqnr asc"
            ),
            mark_published: format!("update {t} set published = $1 where seqnr = ANY($2)"),
        }
    }
}

/// Snapshots table statements.
pub struct SnapshotQueries {
    pub setup: Vec<String>,
    pub get: String,
    pub put: String,
}

impl SnapshotQueries {
    pub fn new(naming: &PGNaming, payload_type: &str) -> Self {
        let t = naming.table("snapshots");
        Self {
            setup: ddl::snapshots_statements(naming, payload_type),
            get: format!("select state, version from {t} where id = $1"),
            put: format!(
                "insert into {t} (id, state, \"version\") values ($1, $2, $3) on conflict (id) do update set version = excluded.version, state = excluded.state"
            ),
        }
    }
}

/// Commands table statements.
pub struct CommandQueries {
    pub setup: Vec<String>,
    pub count: String,
    pub insert: String,
}

impl CommandQueries {
    pub fn new(naming: &PGNaming) -> Self {
        let t = naming.table("commands");
        Self {
            setup: ddl::commands_statements(naming),
            count: format!("select count(*) from {t} where id = $1"),
            insert: format!("insert into {t} (id, address, \"time\") values ($1, $2, $3)"),
        }
    }
}

/// CQRS states table statements.
pub struct StateQueries {
    pub setup: Vec<String>,
    pub get: String,
    pub put: String,
}

impl StateQueries {
    pub fn new(naming: &PGNaming, payload_type: &str) -> Self {
        let t = naming.table("states");
        Self {
            setup: ddl::states_statements(naming, payload_type),
            get: format!("select state, version from {t} where id = $1"),
            put: format!(
                "insert into {t} (id, state, \"version\") values ($1, $2, 1) on conflict (id) do update set version = {t}.version + 1, state = excluded.state where {t}.version = $3"
            ),
        }
    }
}

/// Migrations table and journal rewrite statements.
pub struct MigrationQueries {
    pub create_table: Vec<String>,
    pub select_applied: String,
    pub read_payloads: String,
    pub update_payload: String,
    pub insert_applied: String,
    pub truncate_snapshots: String,
}

impl MigrationQueries {
    pub fn new(naming: &PGNaming) -> Self {
        let migrations = naming.table("migrations");
        let journal = naming.table("journal");
        let snapshots = naming.table("snapshots");
        Self {
            create_table: ddl::migrations_statements(naming),
            select_applied: format!("SELECT \"version\" FROM {migrations}"),
            read_payloads: format!("SELECT id, payload::text FROM {journal} ORDER BY seqnr ASC"),
            update_payload: format!("UPDATE {journal} SET payload = $1::jsonb WHERE id = $2"),
            insert_applied: format!(
                "INSERT INTO {migrations} (\"version\", description) VALUES ($1, $2)"
            ),
            truncate_snapshots: format!("TRUNCATE {snapshots}"),
        }
    }
}

/// `CREATE SCHEMA` for schema-mode naming.
pub fn setup_schema(naming: &PGNaming) -> Vec<String> {
    ddl::schema_statement(naming)
}
