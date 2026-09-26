//! Golden DDL tests: the Rust `PGSchema` output must be byte-for-byte
//! identical to the Scala `PGSchema` output.
//!
//! The golden files under `rust/tests/golden/` are produced by the Scala
//! generator `modules/postgres/src/test/scala/GoldenDDL.scala`:
//!
//! ```text
//! sbt "postgresJVM/Test/runMain edomata.backend.GoldenDDL rust/tests/golden"
//! ```
//!
//! Each file is the list of statements joined by `\n`, plus a trailing
//! newline. Every naming strategy and payload type combination listed in
//! `NAMINGS`, `ES_TYPES` and `CQRS_TYPES` (which mirror the generator) has a file.

use std::path::PathBuf;

use edomata_postgres::{PGNamespace, PGNaming, PGSchema};

fn golden_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../tests/golden")
}

fn naming(kind: &str, ns: &str) -> PGNaming {
    let ns = PGNamespace::try_from(ns).unwrap();
    match kind {
        "schema" => PGNaming::schema(ns),
        "prefixed" => PGNaming::prefixed(ns),
        other => panic!("unknown naming {other}"),
    }
}

/// (naming kind, namespace)
const NAMINGS: &[(&str, &str)] = &[
    ("schema", "auth"),
    ("prefixed", "myapp"),
    ("schema", "Order_v2$"),
    ("prefixed", "order_v2$"),
];

const ES_TYPES: &[(&str, &str, &str, &str)] = &[
    ("jsonb", "jsonb", "jsonb", "jsonb"),
    ("json", "json", "json", "json"),
    ("bytea", "bytea", "bytea", "bytea"),
    ("mixed", "bytea", "json", "jsonb"),
];

const CQRS_TYPES: &[(&str, &str, &str)] = &[
    ("jsonb", "jsonb", "jsonb"),
    ("json", "json", "json"),
    ("bytea", "bytea", "bytea"),
    ("mixed", "json", "bytea"),
];

fn read_golden(name: &str) -> String {
    let path = golden_dir().join(name);
    std::fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("missing golden file {}: {e}", path.display()))
}

fn render(ddl: &[String]) -> String {
    let mut s = ddl.join("\n");
    s.push('\n');
    s
}

#[test]
fn golden_directory_exists() {
    assert!(
        golden_dir().is_dir(),
        "{} not found",
        golden_dir().display()
    );
}

#[test]
fn eventsourcing_ddl_matches_scala_golden_files() {
    let mut checked = 0;
    for (kind, ns) in NAMINGS {
        for (label, event, notif, snapshot) in ES_TYPES {
            let file = format!(
                "eventsourcing_{kind}_{}_{label}.sql",
                ns.to_lowercase().replace('$', "_")
            );
            let expected = read_golden(&file);
            let actual = render(&PGSchema::eventsourcing_with(
                &naming(kind, ns),
                event,
                notif,
                snapshot,
            ));
            assert_eq!(actual, expected, "DDL differs from Scala for {file}");
            checked += 1;
        }
    }
    assert_eq!(checked, NAMINGS.len() * ES_TYPES.len());
}

#[test]
fn cqrs_ddl_matches_scala_golden_files() {
    let mut checked = 0;
    for (kind, ns) in NAMINGS {
        for (label, state, notif) in CQRS_TYPES {
            let file = format!(
                "cqrs_{kind}_{}_{label}.sql",
                ns.to_lowercase().replace('$', "_")
            );
            let expected = read_golden(&file);
            let actual = render(&PGSchema::cqrs_with(&naming(kind, ns), state, notif));
            assert_eq!(actual, expected, "DDL differs from Scala for {file}");
            checked += 1;
        }
    }
    assert_eq!(checked, NAMINGS.len() * CQRS_TYPES.len());
}

#[test]
fn default_payload_type_is_jsonb() {
    let n = naming("prefixed", "myapp");
    assert_eq!(
        PGSchema::eventsourcing(&n),
        PGSchema::eventsourcing_with(&n, "jsonb", "jsonb", "jsonb")
    );
    assert_eq!(
        PGSchema::cqrs(&n),
        PGSchema::cqrs_with(&n, "jsonb", "jsonb")
    );
}

#[test]
fn every_golden_file_is_covered() {
    let expected: usize = NAMINGS.len() * (ES_TYPES.len() + CQRS_TYPES.len());
    let files = std::fs::read_dir(golden_dir())
        .unwrap()
        .filter_map(|e| e.ok())
        .filter(|e| e.path().extension().is_some_and(|x| x == "sql"))
        .filter(|e| {
            let name = e.file_name().to_string_lossy().to_string();
            name.starts_with("eventsourcing_") || name.starts_with("cqrs_")
        })
        .count();
    assert_eq!(
        files, expected,
        "unexpected number of PGSchema golden files"
    );
}
