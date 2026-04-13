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

package edomata.doobie

import _root_.doobie.*
import _root_.doobie.free.connection.ConnectionIO
import _root_.doobie.implicits.*
import _root_.doobie.postgres.implicits.*
import _root_.doobie.util.fragment.Fragment
import cats.effect.kernel.Async
import cats.implicits.*
import edomata.backend.EventMigration
import edomata.backend.MigrationResult
import edomata.backend.PGNaming

import java.util.UUID

/** Automatic event journal migration runner for Doobie.
  *
  * Runs pending migrations on startup, like Flyway for event payloads. Each
  * migration transforms all journal payloads from an old format to a new
  * format. A `migrations` tracking table records which migrations have already
  * been applied, making the process idempotent.
  *
  * {{{
  * for
  *   result <- DoobieMigrations.run(naming, trx, List(v1ToV2, v2ToV3))
  *   _      <- IO.println(s"Applied: \${result.applied}")
  *   driver <- DoobieDriver.from(naming, trx)
  * yield driver
  * }}}
  */
object DoobieMigrations {

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
    * @param trx
    *   Doobie transactor
    * @param migrations
    *   ordered list of migrations to apply
    * @param batchSize
    *   number of rows to process per batch (default 500)
    * @return
    *   result indicating which migrations were applied and which were skipped
    */
  def run[F[_]: Async](
      naming: PGNaming,
      trx: Transactor[F],
      migrations: List[EventMigration],
      batchSize: Int = 500
  ): F[MigrationResult] = {
    val q = MigrationQueries(naming)

    for {
      // Ensure migrations table exists
      _ <- q.createTable.run.transact(trx)

      // Load already-applied versions
      applied <- q.selectApplied.to[List].transact(trx).map(_.toSet)

      // Partition into pending and skipped
      (pending, skipped) = migrations.partition(m =>
        !applied.contains(m.version)
      )

      // Apply each pending migration in its own transaction
      _ <- pending.traverse_ { migration =>
        val program: ConnectionIO[Unit] = for {
          // Read all journal payloads, transform, and batch-update
          rows <- q.readPayloads.to[List]
          transformed <- rows.traverse { case (id, payload) =>
            migration
              .run(payload)
              .leftMap(e =>
                new RuntimeException(
                  s"Migration '${migration.version}' failed for event $id: $e"
                )
              )
              .map(newPayload => (id, newPayload))
              .liftTo[ConnectionIO]
          }
          _ <- transformed
            .grouped(batchSize)
            .toList
            .traverse_ { batch =>
              batch.traverse_ { case (id, newPayload) =>
                q.updatePayload(id, newPayload).run
              }
            }

          // Record this migration as applied
          _ <- q.insertApplied(migration.version, migration.description).run

          // Invalidate all snapshots
          _ <- q.truncateSnapshots.run
        } yield ()

        program.transact(trx)
      }
    } yield MigrationResult(
      applied = pending.map(_.version),
      skipped = skipped.map(_.version)
    )
  }

  private class MigrationQueries(naming: PGNaming) {
    private val migrationsT = Fragment.const(naming.table("migrations"))
    private val journalT = Fragment.const(naming.table("journal"))
    private val snapshotsT = Fragment.const(naming.table("snapshots"))
    private val pk = Fragment.const(naming.constraint("migrations_pk"))

    val createTable: Update0 =
      sql"""CREATE TABLE IF NOT EXISTS $migrationsT (
        "version" text NOT NULL,
        description text NOT NULL,
        applied_at timestamptz NOT NULL DEFAULT now(),
        CONSTRAINT $pk PRIMARY KEY ("version")
      )""".update

    val selectApplied: Query0[String] =
      sql"""SELECT "version" FROM $migrationsT""".query[String]

    val readPayloads: Query0[(UUID, String)] =
      (fr"SELECT id, payload::text FROM" ++ journalT ++ fr"ORDER BY seqnr ASC")
        .query[(UUID, String)]

    private val jsonbCast = Fragment.const("::jsonb")

    def updatePayload(id: UUID, newPayload: String): Update0 =
      (fr"UPDATE" ++ journalT ++ fr"SET payload =" ++ fr"$newPayload" ++ jsonbCast ++ fr"WHERE id = $id").update

    def insertApplied(version: String, description: String): Update0 =
      (fr"""INSERT INTO""" ++ migrationsT ++ fr"""("version", description) VALUES ($version, $description)""").update

    val truncateSnapshots: Update0 =
      (fr"TRUNCATE" ++ snapshotsT).update
  }
}
