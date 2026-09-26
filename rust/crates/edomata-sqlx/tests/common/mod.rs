//! Shared wiring for the sqlx tests: port of `SkunkCompatibilitySuite.scala`
//! / `DoobieCompatibilitySuite.scala` object members.

#![allow(dead_code)]

use std::time::Duration;

use edomata_backend::cqrs::Backend as CqrsBackend;
use edomata_backend::eventsourcing::{Backend, PersistedSnapshotConfig};
use edomata_backend::{Codec, CodecError, PayloadFormat, RetryConfig};
use edomata_backend_tests::cqrs::CqrsBackend as TestCqrsBackend;
use edomata_backend_tests::eventsourcing::EsBackend;
use edomata_backend_tests::{TestCqrsModel, TestDomain, test_cqrs_dsl, test_domain_dsl};
use edomata_sqlx::{PGNaming, PgPool, SqlxCodec, SqlxCqrsDriver, SqlxDriver};
use sqlx::postgres::PgPoolOptions;

/// Connects to `DATABASE_URL` or the docker-compose default.
pub async fn pool() -> PgPool {
    let url = std::env::var("DATABASE_URL")
        .unwrap_or_else(|_| "postgres://postgres:postgres@localhost:5432/postgres".to_string());
    PgPoolOptions::new()
        .max_connections(8)
        .connect(&url)
        .await
        .expect("PostgreSQL from docker-compose must be running (see rust/README.md)")
}

/// Port of the Scala test codecs for `Int`: JSON text (`1234`) in `json` /
/// `jsonb` columns, and the hex-encoded decimal string in `bytea` columns
/// (`1234` → `\x1234`).
#[derive(Clone, Copy, Debug)]
pub struct IntCodec(pub PayloadFormat);

impl Codec<i32> for IntCodec {
    fn format(&self) -> PayloadFormat {
        self.0
    }

    fn encode(&self, value: &i32) -> Result<Vec<u8>, CodecError> {
        let text = value.to_string();
        match self.0 {
            PayloadFormat::Bytea => {
                hex_decode(&text).ok_or_else(|| CodecError::Encode(format!("not hex: {text}")))
            }
            _ => Ok(text.into_bytes()),
        }
    }

    fn decode(&self, bytes: &[u8]) -> Result<i32, CodecError> {
        let text = match self.0 {
            PayloadFormat::Bytea => bytes.iter().map(|b| format!("{b:02x}")).collect::<String>(),
            _ => {
                String::from_utf8(bytes.to_vec()).map_err(|e| CodecError::Decode(e.to_string()))?
            }
        };
        text.trim()
            .parse::<i32>()
            .map_err(|_| CodecError::Decode(format!("Not a number: {text}")))
    }
}

fn hex_decode(text: &str) -> Option<Vec<u8>> {
    if !text.len().is_multiple_of(2) {
        return None;
    }
    (0..text.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&text[i..i + 2], 16).ok())
        .collect()
}

pub fn json_codec() -> SqlxCodec<i32> {
    SqlxCodec::new(IntCodec(PayloadFormat::Json))
}

pub fn jsonb_codec() -> SqlxCodec<i32> {
    SqlxCodec::new(IntCodec(PayloadFormat::Jsonb))
}

pub fn bin_codec() -> SqlxCodec<i32> {
    SqlxCodec::new(IntCodec(PayloadFormat::Bytea))
}

pub fn retry() -> RetryConfig {
    RetryConfig {
        max_retry: 5,
        initial_delay: Duration::from_millis(200),
    }
}

/// Port of `backend(name, codec)`: schema naming, persisted snapshots with
/// no in-memory cache (`maxInMem = 0, maxBuffer = 1`).
pub async fn backend(naming: PGNaming, codec: SqlxCodec<i32>) -> EsBackend {
    let driver = SqlxDriver::new(naming, pool().await).await.unwrap();
    Backend::builder(TestDomain, test_domain_dsl())
        .driver(driver)
        .persisted_snapshot_with(
            codec.clone(),
            PersistedSnapshotConfig {
                size: 0,
                max_buffer: 1,
                ..PersistedSnapshotConfig::default()
            },
        )
        .with_retry_config(retry())
        .build(codec.clone(), codec)
        .await
        .unwrap()
}

/// Port of `backendCqrs(name, codec)`.
pub async fn backend_cqrs(naming: PGNaming, codec: SqlxCodec<i32>) -> TestCqrsBackend {
    let driver = SqlxCqrsDriver::new(naming, pool().await).await.unwrap();
    CqrsBackend::builder(TestCqrsModel, test_cqrs_dsl())
        .driver(driver)
        .with_retry_config(retry())
        .build(codec.clone(), codec)
        .await
        .unwrap()
}

pub fn schema(name: &str) -> PGNaming {
    PGNaming::schema_str(name).unwrap()
}

pub fn prefixed(name: &str) -> PGNaming {
    PGNaming::prefixed_str(name).unwrap()
}
