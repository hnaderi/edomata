//! SQL statements, mirroring `Queries.scala` of the Skunk and Doobie
//! drivers: same tables, columns, constraints, index names and ordering.
//! Setup DDL comes from `edomata_postgres::ddl`, so the tables created by the
//! driver are exactly the ones `PGSchema` generates for migration tools.
//!
//! Each catalogue is built once per storage from a [`PGNaming`] (table,
//! constraint and index names) and a payload SQL type (`jsonb`, `json` or
//! `bytea`, from the codec). Placeholders (`$1`, `$2`...) are documented on
//! each field in bind order. The catalogues are public so that derived
//! drivers (such as `edomata-saas-sqlx`) can reuse them.
//!
//! ```
//! use edomata_sqlx::PGNaming;
//! use edomata_sqlx::queries::JournalQueries;
//!
//! let q = JournalQueries::new(&PGNaming::prefixed_str("accounts").unwrap(), "jsonb");
//! assert!(q.read_stream.contains("from accounts_journal where stream = $1"));
//! ```

use edomata_postgres::{PGNaming, ddl};

/// Journal table statements.
pub struct JournalQueries {
    /// `CREATE TABLE` / `CREATE INDEX` statements of the `journal` table.
    pub setup: Vec<String>,
    /// Appends one event: `$1` id, `$2` stream, `$3` time, `$4` version,
    /// `$5` payload.
    pub insert: String,
    /// Every event, in `seqnr` order.
    pub read_all: String,
    /// Events with `seqnr > $1`, in `seqnr` order.
    pub read_all_after: String,
    /// Events with `seqnr < $1`, in `seqnr` order.
    pub read_all_before: String,
    /// Events of stream `$1`, in version order.
    pub read_stream: String,
    /// Events of stream `$1` with `version > $2`, in version order.
    pub read_stream_after: String,
    /// Events of stream `$1` with `version < $2`, in version order.
    pub read_stream_before: String,
}

const JOURNAL_FIELDS: &str = "id, time, seqnr, version, stream, payload";

impl JournalQueries {
    /// The journal statements for `naming`, with payloads of SQL type
    /// `payload_type`.
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
    /// `CREATE TABLE` / `CREATE INDEX` statements of the `outbox` table.
    pub setup: Vec<String>,
    /// Inserts one item: `$1` payload, `$2` stream, `$3` created,
    /// `$4` correlation, `$5` causation.
    pub insert: String,
    /// Unpublished items (`published is NULL`), in `seqnr` order.
    pub read: String,
    /// Sets `published = $1` on the items whose `seqnr = ANY($2)`.
    pub mark_published: String,
    /// `NOTIFY` channel raised after outbox rows are inserted, if any.
    pub notify_channel: Option<String>,
}

impl OutboxQueries {
    /// The outbox statements, without a `NOTIFY` channel.
    pub fn new(naming: &PGNaming, payload_type: &str) -> Self {
        Self::with_notify(naming, payload_type, None)
    }

    /// The outbox statements, raising `NOTIFY notify_channel` after inserts
    /// when a channel is given.
    pub fn with_notify(
        naming: &PGNaming,
        payload_type: &str,
        notify_channel: Option<&str>,
    ) -> Self {
        let t = naming.table("outbox");
        Self {
            notify_channel: notify_channel.map(str::to_owned),
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
    /// `CREATE TABLE` statement of the `snapshots` table.
    pub setup: Vec<String>,
    /// State and version of stream `$1`.
    pub get: String,
    /// Upserts the snapshot of stream `$1` with state `$2` and version `$3`.
    pub put: String,
}

impl SnapshotQueries {
    /// The snapshot statements, with states of SQL type `payload_type`.
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
    /// `CREATE TABLE` statement of the `commands` table.
    pub setup: Vec<String>,
    /// Counts the commands with id `$1` (0 or 1).
    pub count: String,
    /// Records a command: `$1` id, `$2` address, `$3` time. A duplicate id
    /// violates the primary key.
    pub insert: String,
}

impl CommandQueries {
    /// The command statements for `naming`.
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
    /// `CREATE TABLE` statement of the `states` table.
    pub setup: Vec<String>,
    /// State and version of aggregate `$1`.
    pub get: String,
    /// Inserts aggregate `$1` with state `$2` at version 1, or updates it
    /// and increments its version only if the stored version is `$3`
    /// (optimistic concurrency: zero affected rows means a conflict).
    pub put: String,
}

impl StateQueries {
    /// The state statements, with states of SQL type `payload_type`.
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
    /// `CREATE TABLE IF NOT EXISTS` statement of the `migrations` table.
    pub create_table: Vec<String>,
    /// Versions of the applied migrations.
    pub select_applied: String,
    /// Every journal row's id and payload as text, in `seqnr` order.
    pub read_payloads: String,
    /// Sets the payload of journal row `$2` to `$1` (cast to `jsonb`).
    pub update_payload: String,
    /// Records migration `$1` with description `$2` as applied.
    pub insert_applied: String,
    /// Empties the `snapshots` table.
    pub truncate_snapshots: String,
}

impl MigrationQueries {
    /// The migration statements for `naming`.
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
