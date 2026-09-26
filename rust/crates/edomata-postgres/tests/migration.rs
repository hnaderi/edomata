//! Tests for `EventMigration` (the Scala module has no dedicated suite; the
//! semantics come from the `EventMigration.scala` documentation and the
//! migration runners).

use edomata_postgres::{EventMigration, MigrationResult};
use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize)]
#[serde(tag = "type")]
enum OldEvent {
    Created { name: String },
    Deleted,
}

#[derive(Debug, Serialize)]
#[serde(tag = "type")]
enum NewEvent {
    Created { name: String, email: String },
    Deleted,
}

fn typed() -> EventMigration {
    EventMigration::typed(
        "001",
        "Add email",
        |raw| serde_json::from_str::<OldEvent>(raw).map_err(|e| e.to_string()),
        |old| match old {
            OldEvent::Created { name } => NewEvent::Created {
                name,
                email: String::new(),
            },
            OldEvent::Deleted => NewEvent::Deleted,
        },
        |new| serde_json::to_string(new).unwrap(),
    )
}

#[test]
fn typed_migration_decodes_transforms_and_encodes() {
    let m = typed();
    assert_eq!(m.version, "001");
    assert_eq!(m.description, "Add email");
    assert_eq!(
        m.run(r#"{"type":"Created","name":"bob"}"#),
        Ok(r#"{"type":"Created","name":"bob","email":""}"#.to_string())
    );
    assert_eq!(
        m.run(r#"{"type":"Deleted"}"#),
        Ok(r#"{"type":"Deleted"}"#.to_string())
    );
}

#[test]
fn typed_migration_reports_decoding_failures() {
    let err = typed().run("not json").unwrap_err();
    assert!(err.contains("expected"), "{err}");
}

#[test]
fn and_then_composes_and_takes_the_next_identity() {
    let first = EventMigration::new("001", "first", |raw| Ok(format!("{raw}-1")));
    let second = EventMigration::new("002", "second", |raw| Ok(format!("{raw}-2")));
    let composed = first.and_then(second);
    assert_eq!(composed.version, "002");
    assert_eq!(composed.description, "second");
    assert_eq!(composed.run("x"), Ok("x-1-2".to_string()));
}

#[test]
fn and_then_short_circuits_on_the_first_error() {
    let first = EventMigration::new("001", "first", |_| Err("boom".to_string()));
    let second = EventMigration::new("002", "second", |_| panic!("must not run"));
    assert_eq!(first.and_then(second).run("x"), Err("boom".to_string()));
}

#[test]
fn migration_is_cloneable_and_debuggable() {
    let m = typed();
    let c = m.clone();
    assert_eq!(c.version, m.version);
    assert!(format!("{m:?}").contains("Add email"));
}

#[test]
fn migration_result_defaults_to_empty() {
    assert_eq!(
        MigrationResult::default(),
        MigrationResult {
            applied: vec![],
            skipped: vec![]
        }
    );
}
