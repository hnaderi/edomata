/*
 * Copyright 2021 Beyond Scale Group
 *
 * Licensed under the Apache License, Version 2.0 (the "License");
 * you may not use this file except in compliance with the License.
 * You may obtain a copy of the License at
 *
 *     http://www.apache.org/licenses/LICENSE-2.0
 *
 * Unless required by applicable law or agreed to in writing, software
 * distributed under the License is distributed on an "AS IS" BASIS,
 * WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
 * See the License for the specific language governing permissions and
 * limitations under the License.
 */

package edomata.skunk

import _root_.skunk.*
import _root_.skunk.codec.all.*
import _root_.skunk.implicits.*
import cats.effect.kernel.Async
import cats.effect.kernel.Resource
import cats.implicits.*
import edomata.backend.EventMigration
import edomata.backend.MigrationResult
import edomata.backend.PGNaming

import java.util.UUID

/** Automatic event journal migration runner for Skunk.
  *
  * Runs pending migrations on startup, like Flyway for event payloads. Each
  * migration transforms all journal payloads from an old format to a new
  * format. A `migrations` tracking table records which migrations have already
  * been applied, making the process idempotent.
  *
  * {{{
  * for
  *   result <- SkunkMigrations.run(naming, pool, List(v1ToV2, v2ToV3))
  *   _      <- IO.println(s"Applied: \${result.applied}")
  *   driver <- SkunkDriver.from(naming, pool)
  * yield driver
  * }}}
  */
object SkunkMigrations {

  /** Run all pending event migrations.
    *
    * Creates the migrations tracking table if it does not exist, checks which
    * migrations have already been applied, and applies pending ones in order.
    * Each migration runs in its own transaction for atomicity. Snapshots are
    * truncated after each migration since cached state is invalidated.
    *
    * Safe to call on every application startup.
    *
    * @param naming
    *   the naming strategy for table resolution
    * @param pool
    *   Skunk session pool
    * @param migrations
    *   ordered list of migrations to apply
    * @param batchSize
    *   number of rows to process per batch (default 500)
    * @return
    *   result indicating which migrations were applied and which were skipped
    */
  def run[F[_]: Async](
      naming: PGNaming,
      pool: Resource[F, Session[F]],
      migrations: List[EventMigration],
      batchSize: Int = 500
  ): F[MigrationResult] = {
    val q = MigrationQueries(naming)

    for {
      // Ensure migrations table exists
      _ <- pool.use(_.execute(q.createTable))

      // Load already-applied versions
      applied <- pool.use(
        _.execute(q.selectApplied).map(_.toSet)
      )

      // Partition into pending and skipped
      (pending, skipped) = migrations.partition(m =>
        !applied.contains(m.version)
      )

      // Apply each pending migration in its own transaction
      _ <- pending.traverse_ { migration =>
        pool.flatTap(_.transaction).use { session =>
          for {
            // Stream all journal payloads, transform, and batch-update
            _ <- session.prepare(q.readPayloads).flatMap { ps =>
              ps.stream(Void, batchSize)
                .evalMap { case (id, payload) =>
                  Async[F].fromEither(
                    migration
                      .run(payload)
                      .leftMap(e =>
                        new RuntimeException(
                          s"Migration '${migration.version}' failed for event $id: $e"
                        )
                      )
                      .map(newPayload => (newPayload, id))
                  )
                }
                .chunkN(batchSize)
                .evalMap { chunk =>
                  chunk.toList.traverse_ { case (newPayload, id) =>
                    session
                      .prepare(q.updatePayload)
                      .flatMap(_.execute((newPayload, id)))
                  }
                }
                .compile
                .drain
            }

            // Record this migration as applied
            _ <- session
              .prepare(q.insertApplied)
              .flatMap(_.execute((migration.version, migration.description)))

            // Invalidate all snapshots
            _ <- session.execute(q.truncateSnapshots)
          } yield ()
        }
      }
    } yield MigrationResult(
      applied = pending.map(_.version),
      skipped = skipped.map(_.version)
    )
  }

  private class MigrationQueries(naming: PGNaming) {
    private val migrationsT = naming.table("migrations")
    private val journalT = naming.table("journal")
    private val snapshotsT = naming.table("snapshots")
    private val pk = naming.constraint("migrations_pk")

    val createTable: Command[Void] = sql"""
CREATE TABLE IF NOT EXISTS #$migrationsT (
  "version" text NOT NULL,
  description text NOT NULL,
  applied_at timestamptz NOT NULL DEFAULT now(),
  CONSTRAINT #$pk PRIMARY KEY ("version")
);
""".command

    val selectApplied: Query[Void, String] =
      sql"""SELECT "version" FROM #$migrationsT""".query(text)

    val readPayloads: Query[Void, (UUID, String)] =
      sql"SELECT id, payload::text FROM #$journalT ORDER BY seqnr ASC"
        .query(uuid *: text)

    val updatePayload: Command[(String, UUID)] =
      sql"UPDATE #$journalT SET payload = ${text}::jsonb WHERE id = $uuid".command

    val insertApplied: Command[(String, String)] =
      sql"""INSERT INTO #$migrationsT ("version", description) VALUES ($text, $text)""".command

    val truncateSnapshots: Command[Void] =
      sql"TRUNCATE #$snapshotsT".command
  }
}
