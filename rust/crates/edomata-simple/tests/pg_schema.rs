//! Port of `JPGSchemaSuite.scala`.

use edomata_simple::{SimpleError, SimplePGSchema};

#[test]
fn eventsourcing_generates_ddl_statements() {
    let ddl = SimplePGSchema::eventsourcing("myapp").unwrap();
    assert!(!ddl.is_empty());
    let all = ddl.join("\n");
    assert!(
        all.contains("myapp_journal"),
        "Missing journal table in: {all}"
    );
    assert!(
        all.contains("myapp_outbox"),
        "Missing outbox table in: {all}"
    );
    assert!(
        all.contains("myapp_commands"),
        "Missing commands table in: {all}"
    );
    assert!(
        all.contains("myapp_snapshots"),
        "Missing snapshots table in: {all}"
    );
}

#[test]
fn eventsourcing_with_custom_types() {
    let all = SimplePGSchema::eventsourcing_with("test", "json", "json", "bytea")
        .unwrap()
        .join("\n");
    assert!(all.contains("json"));
    assert!(all.contains("bytea"));
}

#[test]
fn cqrs_generates_ddl_statements() {
    let ddl = SimplePGSchema::cqrs("myapp").unwrap();
    assert!(!ddl.is_empty());
    let all = ddl.join("\n");
    assert!(
        all.contains("myapp_states"),
        "Missing states table in: {all}"
    );
    assert!(
        all.contains("myapp_outbox"),
        "Missing outbox table in: {all}"
    );
    assert!(
        all.contains("myapp_commands"),
        "Missing commands table in: {all}"
    );
    let all = SimplePGSchema::cqrs_with("myapp", "bytea", "json")
        .unwrap()
        .join("\n");
    assert!(all.contains("state bytea NOT NULL"));
    assert!(all.contains("payload json NOT NULL"));
}

#[test]
fn eventsourcing_with_schema_generates_schema_ddl() {
    let all = SimplePGSchema::eventsourcing_with_schema("auth")
        .unwrap()
        .join("\n");
    assert!(
        all.contains("CREATE SCHEMA"),
        "Missing CREATE SCHEMA in: {all}"
    );
    assert!(
        all.contains("\"auth\".journal"),
        "Missing schema-qualified table in: {all}"
    );
}

#[test]
fn cqrs_with_schema_generates_schema_ddl() {
    let all = SimplePGSchema::cqrs_with_schema("auth").unwrap().join("\n");
    assert!(all.contains("CREATE SCHEMA"));
}

#[test]
fn invalid_namespace_is_an_error() {
    let err = SimplePGSchema::eventsourcing("").unwrap_err();
    assert!(matches!(err, SimpleError::InvalidNamespace(_)), "{err:?}");
    assert!(err.to_string().starts_with("Invalid namespace: "), "{err}");
    assert!(SimplePGSchema::cqrs("1-bad").is_err());
}
