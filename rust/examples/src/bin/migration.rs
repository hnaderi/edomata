//! Port of `examples/src/main/scala/MigrationExample.scala`: evolving event
//! schemas with journal migrations. Events stored in the journal are
//! rewritten in place, like Flyway for event payloads.
//!
//! Scenario, a product catalogue where:
//! - V1 → V2: `PriceUpdated` gains a `currency` field (defaulting to "USD")
//! - V2 → V3: `Created` gains a `description` field (defaulting to "")
//!
//! Unlike the Scala example, this one also seeds a V1 journal first and reads
//! it back with the V3 codec after the migrations ran.
//!
//! ```sh
//! cargo run -p edomata-examples --bin migration
//! ```

use edomata_backend::eventsourcing::Backend;
use edomata_core::*;
use edomata_examples::{command, connect};
use edomata_postgres::{EventMigration, PGNaming};
use edomata_sqlx::{SqlxCodec, SqlxDriver, SqlxMigrations};
use futures::TryStreamExt;
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};

// ---------------------------------------------------------------------------
// Section 1: EVENT VERSIONS
//
// Old event types are kept so that the compiler can verify that every
// migration handles ALL cases (exhaustive `match`). Once the migration has
// run in all environments, old types can be deleted. The JSON shape is the
// one Circe gives Scala 3 enums (`{"Created":{...}}`, `{"Archived":{}}`).
// ---------------------------------------------------------------------------

mod v1 {
    use super::*;

    #[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
    #[serde(rename_all_fields = "camelCase")]
    pub enum Event {
        Created { name: String, price_cents: i64 },
        PriceUpdated { price_cents: i64 },
        Archived {},
    }
}

mod v2 {
    use super::*;

    #[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
    #[serde(rename_all_fields = "camelCase")]
    pub enum Event {
        Created { name: String, price_cents: i64 },
        PriceUpdated { price_cents: i64, currency: String },
        Archived {},
    }
}

/// V3 (current): the type used by the running application; all journal
/// entries are in this format after the migrations ran.
mod v3 {
    use super::*;

    #[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
    #[serde(rename_all_fields = "camelCase")]
    pub enum Event {
        Created {
            name: String,
            price_cents: i64,
            description: String,
        },
        PriceUpdated {
            price_cents: i64,
            currency: String,
        },
        Archived {},
    }
}

// ---------------------------------------------------------------------------
// Section 2: MIGRATION DEFINITIONS
//
// `EventMigration::typed` decodes the old type, transforms it and encodes
// the new one. Adding a case to `v1::Event` without handling it here fails
// to compile.
// ---------------------------------------------------------------------------

fn decode<A: DeserializeOwned>(raw: &str) -> Result<A, String> {
    serde_json::from_str(raw).map_err(|e| e.to_string())
}

fn encode<B: Serialize>(b: &B) -> String {
    serde_json::to_string(b).expect("events serialize")
}

/// Migration 001: add `currency` to `PriceUpdated`, defaulting to "USD".
fn v1_to_v2() -> EventMigration {
    EventMigration::typed(
        "001",
        "Add currency to PriceUpdated",
        decode::<v1::Event>,
        |e| match e {
            v1::Event::Created { name, price_cents } => v2::Event::Created { name, price_cents },
            v1::Event::PriceUpdated { price_cents } => v2::Event::PriceUpdated {
                price_cents,
                currency: "USD".to_string(),
            },
            v1::Event::Archived {} => v2::Event::Archived {},
        },
        encode,
    )
}

/// Migration 002: add `description` to `Created`, defaulting to "".
fn v2_to_v3() -> EventMigration {
    EventMigration::typed(
        "002",
        "Add description to Created",
        decode::<v2::Event>,
        |e| match e {
            v2::Event::Created { name, price_cents } => v3::Event::Created {
                name,
                price_cents,
                description: String::new(),
            },
            v2::Event::PriceUpdated {
                price_cents,
                currency,
            } => v3::Event::PriceUpdated {
                price_cents,
                currency,
            },
            v2::Event::Archived {} => v3::Event::Archived {},
        },
        encode,
    )
}

// ---------------------------------------------------------------------------
// Section 3: A MINIMAL MODEL, parameterised by the event version
// ---------------------------------------------------------------------------

/// Counts events; generic so that the same model writes V1 and reads V3.
struct Counting<E>(std::marker::PhantomData<E>);

impl<E> Counting<E> {
    fn new() -> Self {
        Self(std::marker::PhantomData)
    }
}

impl<E: Clone + Send + Sync + 'static> DomainModel for Counting<E> {
    type State = u32;
    type Event = E;
    type Rejection = String;

    fn initial(&self) -> u32 {
        0
    }

    fn transition(&self, _event: &E, state: u32) -> Result<u32, NonEmpty<String>> {
        Ok(state + 1)
    }
}

// ---------------------------------------------------------------------------
// Section 4: APPLICATION STARTUP
//
// Migrations run before the backend is built. Already-applied migrations
// are skipped (tracked in the `products_migrations` table) and snapshots
// are truncated after each migration so that states are rebuilt from the
// migrated journal.
// ---------------------------------------------------------------------------

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // A fresh namespace per run: a V1 journal written after the migrations
    // were applied would not be migrated (migrations run once per
    // namespace), so the scenario starts from an empty journal every time.
    let namespace = format!(
        "products_{}",
        &uuid::Uuid::new_v4().simple().to_string()[..8]
    );
    let naming = PGNaming::prefixed_str(&namespace)?;
    println!("namespace: {namespace}");
    let pool = connect().await?;

    // Seed a V1 journal (an old version of the application writing events).
    let v1_backend = Backend::builder(
        Counting::<v1::Event>::new(),
        DomainDsl::<(), u32, v1::Event, String, ()>::new(),
    )
    .driver(SqlxDriver::new(naming.clone(), pool.clone()).await?)
    // The migration runner truncates the snapshots table (as in Scala), so
    // the application must use persisted snapshots (or create the table
    // with `PGSchema`).
    .persisted_snapshot(SqlxCodec::<u32>::jsonb())
    .build_default()
    .await?;
    let dsl = DomainDsl::<(), u32, v1::Event, String, ()>::new();
    let writer = v1_backend.compile(dsl.accept(nonempty![
        v1::Event::Created {
            name: "Widget".to_string(),
            price_cents: 1999
        },
        v1::Event::PriceUpdated { price_cents: 2499 },
        v1::Event::Archived {}
    ]));
    let product = uuid::Uuid::new_v4().to_string();
    writer(command(&product, ()))
        .await?
        .map_err(|rejections| format!("rejected: {rejections:?}"))?;
    v1_backend.close().await?;
    println!("wrote 3 V1 events for {product}");

    // Run migrations: idempotent, safe to call on every startup.
    let migrations = [v1_to_v2(), v2_to_v3()];
    let result = SqlxMigrations::run(&naming, &pool, &migrations).await?;
    println!(
        "Migrations applied: {:?}, skipped: {:?}",
        result.applied, result.skipped
    );
    // Running them again (the next startup) applies nothing.
    let again = SqlxMigrations::run(&naming, &pool, &migrations).await?;
    println!(
        "Second run applied: {:?}, skipped: {:?}",
        again.applied, again.skipped
    );

    // Now build the backend with the V3 codec only: the journal is guaranteed
    // to contain V3 events after the migrations completed.
    let v3_backend = Backend::builder(
        Counting::<v3::Event>::new(),
        DomainDsl::<(), u32, v3::Event, String, ()>::new(),
    )
    .driver(SqlxDriver::new(naming, pool).await?)
    .persisted_snapshot(SqlxCodec::<u32>::jsonb())
    .build_default()
    .await?;
    let events: Vec<v3::Event> = v3_backend
        .journal()
        .read_stream(&product)
        .map_ok(|e| e.payload)
        .try_collect()
        .await?;
    println!("V3 journal of {product}:");
    for e in &events {
        println!("  {e:?}");
    }
    assert_eq!(
        events,
        vec![
            v3::Event::Created {
                name: "Widget".to_string(),
                price_cents: 1999,
                description: String::new()
            },
            v3::Event::PriceUpdated {
                price_cents: 2499,
                currency: "USD".to_string()
            },
            v3::Event::Archived {}
        ]
    );
    v3_backend.close().await?;
    println!("Backend ready with V3 event format");
    Ok(())
}
