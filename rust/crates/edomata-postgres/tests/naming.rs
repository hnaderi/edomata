//! Port of `PGNamespaceSuite.scala` (`PGNamespaceSuite` and `PGSchemaSuite`).

use edomata_postgres::{PGNamespace, PGNamespaceError, PGNaming, PGSchema};

fn ns(s: &str) -> PGNamespace {
    PGNamespace::try_from(s).unwrap()
}

// --- PGNamespaceSuite -------------------------------------------------------

#[test]
fn constructor() {
    // Scala validates at compile time with a macro; Rust validates at
    // construction and reports the same message.
    ns("a");
    assert_eq!(
        PGNamespace::try_from("").unwrap_err().to_string(),
        "Name: \"\" does not match ([A-Za-z_][A-Za-z_0-9$]*)"
    );
    assert!(matches!(
        PGNamespace::try_from("1a"),
        Err(PGNamespaceError::Invalid(_))
    ));
    assert!(PGNamespace::try_from("a-b").is_err());
    assert!(PGNamespace::try_from("a b").is_err());
    assert!(PGNamespace::try_from("$a").is_err());
    assert!(PGNamespace::try_from("_ok$9").is_ok());
    assert_eq!("abc".parse::<PGNamespace>().unwrap().as_str(), "abc");
}

#[test]
fn length_limit() {
    let long = "a".repeat(64);
    assert_eq!(
        PGNamespace::from_string(&long),
        Err(PGNamespaceError::TooLong(64))
    );
    assert_eq!(
        PGNamespace::from_string(&long).unwrap_err().to_string(),
        "Name is too long: 64 (max allowed is 63)"
    );
    assert!(PGNamespace::from_string(&"a".repeat(63)).is_ok());
}

#[test]
fn schema_produces_schema_qualified_table_names() {
    let naming = PGNaming::schema(ns("auth"));
    assert_eq!(naming.table("journal"), "\"auth\".journal");
    assert_eq!(naming.table("outbox"), "\"auth\".outbox");
    assert_eq!(naming.table("snapshots"), "\"auth\".snapshots");
    assert_eq!(naming.table("commands"), "\"auth\".commands");
    assert_eq!(naming.table("states"), "\"auth\".states");
}

#[test]
fn schema_does_not_prefix_constraint_or_index_names() {
    let naming = PGNaming::schema(ns("auth"));
    assert_eq!(naming.constraint("journal_pk"), "journal_pk");
    assert_eq!(naming.index("journal_seqnr_idx"), "journal_seqnr_idx");
}

#[test]
fn schema_needs_schema_setup() {
    assert!(PGNaming::schema(ns("auth")).needs_schema_setup());
}

#[test]
fn prefixed_produces_prefixed_table_names() {
    let naming = PGNaming::prefixed(ns("auth"));
    assert_eq!(naming.table("journal"), "auth_journal");
    assert_eq!(naming.table("outbox"), "auth_outbox");
    assert_eq!(naming.table("snapshots"), "auth_snapshots");
    assert_eq!(naming.table("commands"), "auth_commands");
    assert_eq!(naming.table("states"), "auth_states");
}

#[test]
fn prefixed_prefixes_constraint_and_index_names() {
    let naming = PGNaming::prefixed(ns("auth"));
    assert_eq!(naming.constraint("journal_pk"), "auth_journal_pk");
    assert_eq!(naming.index("journal_seqnr_idx"), "auth_journal_seqnr_idx");
}

#[test]
fn prefixed_does_not_need_schema_setup() {
    assert!(!PGNaming::prefixed(ns("auth")).needs_schema_setup());
}

#[test]
fn namespace_prefixed_convenience() {
    let naming = ns("auth").prefixed();
    assert_eq!(naming.table("journal"), "auth_journal");
    assert!(!naming.needs_schema_setup());
    assert_eq!(ns("auth").schema(), PGNaming::schema(ns("auth")));
}

#[test]
fn naming_string_constructors() {
    let schema = PGNaming::schema_str("auth").unwrap();
    assert_eq!(schema.table("journal"), "\"auth\".journal");
    let prefixed = PGNaming::prefixed_str("auth").unwrap();
    assert_eq!(prefixed.table("journal"), "auth_journal");
    assert!(PGNaming::schema_str("").is_err());
    assert_eq!(prefixed.namespace().as_str(), "auth");
}

// --- PGSchemaSuite ----------------------------------------------------------

#[test]
fn eventsourcing_ddl_with_schema_mode_includes_create_schema() {
    let ddl = PGSchema::eventsourcing(&PGNaming::schema(ns("auth")));
    assert!(ddl[0].contains("CREATE SCHEMA IF NOT EXISTS \"auth\""));
}

#[test]
fn eventsourcing_ddl_with_prefix_mode_omits_create_schema() {
    let ddl = PGSchema::eventsourcing(&PGNaming::prefixed(ns("auth")));
    assert!(ddl.iter().all(|s| !s.contains("CREATE SCHEMA")));
}

#[test]
fn eventsourcing_ddl_creates_journal_outbox_commands_snapshots_tables() {
    let all = PGSchema::eventsourcing(&PGNaming::prefixed(ns("myapp"))).join("\n");
    assert!(all.contains("CREATE TABLE IF NOT EXISTS myapp_journal"));
    assert!(all.contains("CREATE TABLE IF NOT EXISTS myapp_outbox"));
    assert!(all.contains("CREATE TABLE IF NOT EXISTS myapp_commands"));
    assert!(all.contains("CREATE TABLE IF NOT EXISTS myapp_snapshots"));
}

#[test]
fn eventsourcing_ddl_creates_indexes_for_journal() {
    let all = PGSchema::eventsourcing(&PGNaming::prefixed(ns("myapp"))).join("\n");
    assert!(all.contains("CREATE INDEX IF NOT EXISTS myapp_journal_seqnr_idx"));
    assert!(all.contains("CREATE INDEX IF NOT EXISTS myapp_journal_stream_idx"));
}

#[test]
fn eventsourcing_ddl_uses_prefixed_constraint_names() {
    let all = PGSchema::eventsourcing(&PGNaming::prefixed(ns("myapp"))).join("\n");
    assert!(all.contains("CONSTRAINT myapp_journal_pk PRIMARY KEY"));
    assert!(all.contains("CONSTRAINT myapp_journal_un UNIQUE"));
    assert!(all.contains("CONSTRAINT myapp_outbox_pk PRIMARY KEY"));
    assert!(all.contains("CONSTRAINT myapp_commands_pk PRIMARY KEY"));
    assert!(all.contains("CONSTRAINT myapp_snapshots_pk PRIMARY KEY"));
}

#[test]
fn eventsourcing_ddl_uses_schema_qualified_table_names_in_schema_mode() {
    let all = PGSchema::eventsourcing(&PGNaming::schema(ns("auth"))).join("\n");
    assert!(all.contains("\"auth\".journal"));
    assert!(all.contains("\"auth\".outbox"));
    assert!(all.contains("\"auth\".commands"));
    assert!(all.contains("\"auth\".snapshots"));
}

#[test]
fn eventsourcing_ddl_uses_custom_payload_types() {
    let all =
        PGSchema::eventsourcing_with(&PGNaming::prefixed(ns("myapp")), "bytea", "json", "jsonb")
            .join("\n");
    assert!(all.contains("myapp_journal") && all.contains("payload bytea NOT NULL"));
    assert!(all.contains("myapp_outbox") && all.contains("payload json NOT NULL"));
    assert!(all.contains("myapp_snapshots") && all.contains("state jsonb NOT NULL"));
}

#[test]
fn cqrs_ddl_creates_states_outbox_commands_tables() {
    let all = PGSchema::cqrs(&PGNaming::prefixed(ns("myapp"))).join("\n");
    assert!(all.contains("CREATE TABLE IF NOT EXISTS myapp_states"));
    assert!(all.contains("CREATE TABLE IF NOT EXISTS myapp_outbox"));
    assert!(all.contains("CREATE TABLE IF NOT EXISTS myapp_commands"));
    assert!(!all.contains("myapp_journal"));
    assert!(!all.contains("myapp_snapshots"));
}

#[test]
fn cqrs_ddl_with_schema_mode_includes_create_schema() {
    let ddl = PGSchema::cqrs(&PGNaming::schema(ns("auth")));
    assert!(ddl[0].contains("CREATE SCHEMA IF NOT EXISTS \"auth\""));
}

#[test]
fn ddl_statements_are_valid_standalone_sql() {
    for stmt in PGSchema::eventsourcing(&PGNaming::prefixed(ns("test"))) {
        assert!(
            stmt.ends_with(';'),
            "Statement does not end with semicolon: {stmt}"
        );
    }
}
