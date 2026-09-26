//! Event payload migrations, like Flyway for journal payloads.

use edomata_backend::BackendError;
use edomata_postgres::{EventMigration, MigrationResult, PGNaming};
use sqlx::{PgPool, Row};
use uuid::Uuid;

use crate::error::map_sqlx;
use crate::eventsourcing::execute_all;
use crate::queries::MigrationQueries;

/// Default number of rows updated per batch.
pub const DEFAULT_MIGRATION_BATCH_SIZE: usize = 500;

/// Runs pending [`EventMigration`]s. Mirrors Scala's `SkunkMigrations` /
/// `DoobieMigrations`.
///
/// `run` creates the `migrations` tracking table if needed (even with
/// `skip_setup` drivers, as in Scala), reads the applied versions, and
/// applies the pending migrations in list order. Each migration rewrites
/// every journal payload in its own transaction, records itself in the
/// tracking table and truncates the snapshots (cached state is invalid
/// after a payload change). Already-applied versions are skipped, so
/// calling `run` on every start-up is safe.
#[derive(Clone, Copy, Debug, Default)]
pub struct SqlxMigrations;

impl SqlxMigrations {
    /// Runs the pending migrations with the default batch size.
    pub async fn run(
        naming: &PGNaming,
        pool: &PgPool,
        migrations: &[EventMigration],
    ) -> Result<MigrationResult, BackendError> {
        Self::run_with_batch_size(naming, pool, migrations, DEFAULT_MIGRATION_BATCH_SIZE).await
    }

    /// Runs the pending migrations, updating `batch_size` rows at a time.
    pub async fn run_with_batch_size(
        naming: &PGNaming,
        pool: &PgPool,
        migrations: &[EventMigration],
        batch_size: usize,
    ) -> Result<MigrationResult, BackendError> {
        let q = MigrationQueries::new(naming);
        execute_all(pool, &q.create_table).await?;
        let applied: Vec<String> = sqlx::query_scalar(&q.select_applied)
            .fetch_all(pool)
            .await
            .map_err(map_sqlx)?;
        let (pending, skipped): (Vec<_>, Vec<_>) = migrations
            .iter()
            .partition(|m| !applied.contains(&m.version));

        for migration in &pending {
            let mut tx = pool.begin().await.map_err(map_sqlx)?;
            let rows = sqlx::query(&q.read_payloads)
                .fetch_all(&mut *tx)
                .await
                .map_err(map_sqlx)?;
            let mut transformed = Vec::with_capacity(rows.len());
            for row in &rows {
                let id: Uuid = row.try_get("id").map_err(map_sqlx)?;
                let payload: String = row.try_get("payload").map_err(map_sqlx)?;
                let new_payload = migration.run(&payload).map_err(|e| {
                    BackendError::persistence(format!(
                        "Migration '{}' failed for event {id}: {e}",
                        migration.version
                    ))
                })?;
                transformed.push((id, new_payload));
            }
            for batch in transformed.chunks(batch_size.max(1)) {
                for (id, new_payload) in batch {
                    sqlx::query(&q.update_payload)
                        .bind(new_payload)
                        .bind(id)
                        .execute(&mut *tx)
                        .await
                        .map_err(map_sqlx)?;
                }
            }
            sqlx::query(&q.insert_applied)
                .bind(&migration.version)
                .bind(&migration.description)
                .execute(&mut *tx)
                .await
                .map_err(map_sqlx)?;
            sqlx::query(&q.truncate_snapshots)
                .execute(&mut *tx)
                .await
                .map_err(map_sqlx)?;
            tx.commit().await.map_err(map_sqlx)?;
            tracing::info!(version = %migration.version, rows = rows.len(), "applied event migration");
        }

        Ok(MigrationResult {
            applied: pending.iter().map(|m| m.version.clone()).collect(),
            skipped: skipped.iter().map(|m| m.version.clone()).collect(),
        })
    }
}
