//! Samples of the "Event migrations" chapter.

use edomata_postgres::{EventMigration, PGNaming};
use serde::{Deserialize, Serialize};

// ANCHOR: versions
/// Old event type, kept so that the compiler checks the migration is total.
pub mod v1 {
    use super::*;

    #[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
    pub enum Event {
        Created { name: String },
        PriceUpdated { price: i64 },
    }
}

/// New event type.
pub mod v2 {
    use super::*;

    #[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
    pub enum Event {
        Created { name: String },
        PriceUpdated { price: i64, currency: String },
    }
}
// ANCHOR_END: versions

// ANCHOR: migration
/// Migration 001: `PriceUpdated` gains a `currency`, defaulting to "USD".
/// `transform` is a `match` over `v1::Event`: forgetting a variant does not compile.
pub fn v1_to_v2() -> EventMigration {
    EventMigration::typed(
        "001",
        "Add currency to PriceUpdated",
        |raw| serde_json::from_str::<v1::Event>(raw).map_err(|e| e.to_string()),
        |event| match event {
            v1::Event::Created { name } => v2::Event::Created { name },
            v1::Event::PriceUpdated { price } => v2::Event::PriceUpdated {
                price,
                currency: "USD".to_string(),
            },
        },
        |event| serde_json::to_string(event).expect("events serialize"),
    )
}
// ANCHOR_END: migration

// ANCHOR: chaining
pub fn all_migrations() -> Vec<EventMigration> {
    vec![v1_to_v2(), v2_to_v3()]
}

pub fn v2_to_v3() -> EventMigration {
    // Operating on the JSON text directly (no compile-time exhaustivity).
    EventMigration::new("002", "Add description to Created", |raw| {
        let mut value: serde_json::Value = serde_json::from_str(raw).map_err(|e| e.to_string())?;
        if let Some(created) = value.get_mut("Created") {
            created["description"] = serde_json::Value::String(String::new());
        }
        Ok(value.to_string())
    })
}

/// Two migrations composed into one.
pub fn v1_to_v3() -> EventMigration {
    v1_to_v2().and_then(v2_to_v3())
}
// ANCHOR_END: chaining

// ANCHOR: running
/// Run the migrations before building the backend: idempotent, safe on every startup.
pub async fn run_migrations(
    pool: edomata_sqlx::PgPool,
) -> Result<(), edomata_backend::BackendError> {
    use edomata_sqlx::{SqlxDriver, SqlxMigrations};

    let naming = PGNaming::prefixed_str("products")
        .map_err(|e| edomata_backend::BackendError::persistence(e.to_string()))?;
    let result = SqlxMigrations::run(&naming, &pool, &all_migrations()).await?;
    println!(
        "Applied: {:?}, skipped: {:?}",
        result.applied, result.skipped
    );
    // Now build the backend with the latest event codec only.
    let _driver = SqlxDriver::new(naming, pool).await?;
    Ok(())
}
// ANCHOR_END: running

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn migrations_rewrite_payloads() {
        // ANCHOR: migration_tests
        let migrated = v1_to_v2()
            .run(r#"{"PriceUpdated":{"price":1999}}"#)
            .unwrap();
        assert_eq!(
            migrated,
            r#"{"PriceUpdated":{"price":1999,"currency":"USD"}}"#
        );
        let created = v1_to_v3().run(r#"{"Created":{"name":"Widget"}}"#).unwrap();
        assert_eq!(created, r#"{"Created":{"description":"","name":"Widget"}}"#);
        assert!(v1_to_v2().run("not json").is_err());
        // ANCHOR_END: migration_tests
    }
}
