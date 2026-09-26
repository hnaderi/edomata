//! Shared helpers for the examples (`src/bin/*.rs`), which are ports of
//! the Scala programs under `examples/src/main/scala/`.
//!
//! Every example connects to the PostgreSQL instance of the repository's
//! `docker-compose.yml` (or to `DATABASE_URL`) and can be run with
//! `cargo run -p edomata-examples --bin <name>`.

#![forbid(unsafe_code)]

use chrono::{DateTime, Utc};
use edomata_core::CommandMessage;
use edomata_sqlx::PgPool;
use sqlx::postgres::PgPoolOptions;

/// The docker-compose connection URL.
pub const DEFAULT_DATABASE_URL: &str = "postgres://postgres:postgres@localhost:5432/postgres";

/// Connects to `DATABASE_URL`, or to the docker-compose instance.
pub async fn connect() -> Result<PgPool, sqlx::Error> {
    let url = std::env::var("DATABASE_URL").unwrap_or_else(|_| DEFAULT_DATABASE_URL.to_string());
    PgPoolOptions::new().max_connections(10).connect(&url).await
}

/// A command message with a random id, issued now.
pub fn command<C>(address: &str, payload: C) -> CommandMessage<C> {
    CommandMessage::new(uuid::Uuid::new_v4().to_string(), now(), address, payload)
}

/// The current time.
pub fn now() -> DateTime<Utc> {
    Utc::now()
}
