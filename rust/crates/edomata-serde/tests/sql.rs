//! SQL test against the docker-compose PostgreSQL: payloads bound with the
//! wire wrappers are stored as real `jsonb` / `json` / `bytea` values, are
//! queryable with `->>` and `@>`, and read back through `SerdeCodec`.
//!
//! Connection: `DATABASE_URL`, or the docker-compose default
//! `postgres://postgres:postgres@localhost:5432/postgres`.

#![cfg(feature = "sqlx")]

use edomata_backend::{Codec, PayloadFormat};
use edomata_serde::SerdeCodec;
use edomata_serde::pg::{ByteaPayload, JsonPayload, JsonbPayload, PgPayload};
use serde::{Deserialize, Serialize};
use sqlx::postgres::PgPoolOptions;
use sqlx::{PgPool, Row};

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
struct Opened {
    owner: String,
    amount: i64,
    tags: Vec<String>,
}

fn sample() -> Opened {
    Opened {
        owner: "bob".into(),
        amount: 100,
        tags: vec!["vip".into(), "eu".into()],
    }
}

async fn pool() -> PgPool {
    let url = std::env::var("DATABASE_URL")
        .unwrap_or_else(|_| "postgres://postgres:postgres@localhost:5432/postgres".to_string());
    PgPoolOptions::new()
        .max_connections(2)
        .connect(&url)
        .await
        .expect("PostgreSQL from docker-compose must be running (see rust/README.md)")
}

async fn fresh_table(pool: &PgPool, name: &str, payload_type: &str) {
    sqlx::query(&format!("DROP TABLE IF EXISTS {name}"))
        .execute(pool)
        .await
        .unwrap();
    sqlx::query(&format!(
        "CREATE TABLE {name} (id text PRIMARY KEY, payload {payload_type} NOT NULL)"
    ))
    .execute(pool)
    .await
    .unwrap();
}

#[tokio::test]
async fn jsonb_payload_is_stored_as_jsonb_and_queryable() {
    let pool = pool().await;
    let table = "edomata_serde_test_jsonb";
    fresh_table(&pool, table, "jsonb").await;
    let codec = SerdeCodec::<Opened>::jsonb();
    let bytes = codec.encode(&sample()).unwrap();

    sqlx::query(&format!(
        "INSERT INTO {table} (id, payload) VALUES ($1, $2)"
    ))
    .bind("a")
    .bind(JsonbPayload(bytes.clone()))
    .execute(&pool)
    .await
    .unwrap();

    // ->> extracts a field as text
    let owner: String = sqlx::query_scalar(&format!(
        "SELECT payload->>'owner' FROM {table} WHERE id = $1"
    ))
    .bind("a")
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(owner, "bob");

    // @> containment works on jsonb
    let contains: bool = sqlx::query_scalar(&format!(
        "SELECT payload @> '{{\"tags\":[\"vip\"]}}'::jsonb FROM {table} WHERE id = $1"
    ))
    .bind("a")
    .fetch_one(&pool)
    .await
    .unwrap();
    assert!(contains);

    // the column type really is jsonb
    let type_name: String = sqlx::query_scalar(&format!(
        "SELECT pg_typeof(payload)::text FROM {table} LIMIT 1"
    ))
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(type_name, "jsonb");

    // read back through the wrapper and the codec
    let row = sqlx::query(&format!("SELECT payload FROM {table} WHERE id = $1"))
        .bind("a")
        .fetch_one(&pool)
        .await
        .unwrap();
    let stored: JsonbPayload = row.try_get("payload").unwrap();
    assert_eq!(codec.decode(&stored.0).unwrap(), sample());
    // jsonb normalises key order/whitespace but the compact encoding is stable here
    assert_eq!(
        serde_json::from_slice::<serde_json::Value>(&stored.0).unwrap(),
        serde_json::from_slice::<serde_json::Value>(&bytes).unwrap()
    );

    // PgPayload decodes by column type
    let any: PgPayload = row.try_get("payload").unwrap();
    assert_eq!(any.format(), PayloadFormat::Jsonb);
    assert_eq!(codec.decode(any.bytes()).unwrap(), sample());
}

#[tokio::test]
async fn json_payload_is_stored_as_json_and_queryable() {
    let pool = pool().await;
    let table = "edomata_serde_test_json";
    fresh_table(&pool, table, "json").await;
    let codec = SerdeCodec::<Opened>::json();
    let bytes = codec.encode(&sample()).unwrap();

    sqlx::query(&format!(
        "INSERT INTO {table} (id, payload) VALUES ($1, $2)"
    ))
    .bind("a")
    .bind(PgPayload::new(PayloadFormat::Json, bytes.clone()))
    .execute(&pool)
    .await
    .unwrap();

    let amount: String = sqlx::query_scalar(&format!("SELECT payload->>'amount' FROM {table}"))
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(amount, "100");

    let type_name: String =
        sqlx::query_scalar(&format!("SELECT pg_typeof(payload)::text FROM {table}"))
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(type_name, "json");

    let row = sqlx::query(&format!("SELECT payload FROM {table}"))
        .fetch_one(&pool)
        .await
        .unwrap();
    let stored: JsonPayload = row.try_get("payload").unwrap();
    // json keeps the text verbatim
    assert_eq!(stored.0, bytes);
    assert_eq!(codec.decode(&stored.0).unwrap(), sample());
}

#[tokio::test]
async fn bytea_payload_round_trips() {
    let pool = pool().await;
    let table = "edomata_serde_test_bytea";
    fresh_table(&pool, table, "bytea").await;
    let codec = SerdeCodec::<Opened>::bytea();
    let bytes = codec.encode(&sample()).unwrap();

    sqlx::query(&format!(
        "INSERT INTO {table} (id, payload) VALUES ($1, $2)"
    ))
    .bind("a")
    .bind(ByteaPayload(bytes.clone()))
    .execute(&pool)
    .await
    .unwrap();

    let row = sqlx::query(&format!("SELECT payload FROM {table}"))
        .fetch_one(&pool)
        .await
        .unwrap();
    let stored: ByteaPayload = row.try_get("payload").unwrap();
    assert_eq!(stored.0, bytes);
    assert_eq!(codec.decode(&stored.0).unwrap(), sample());
    let any: PgPayload = row.try_get("payload").unwrap();
    assert_eq!(any.format(), PayloadFormat::Bytea);

    // bytea JSON is still queryable after a cast
    let owner: String = sqlx::query_scalar(&format!(
        "SELECT convert_from(payload, 'UTF8')::jsonb->>'owner' FROM {table}"
    ))
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(owner, "bob");
}
