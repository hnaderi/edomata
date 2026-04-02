/*
 * Copyright 2021 Hossein Naderi
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

package edomata.examples.migrations

import cats.effect.IO
import cats.effect.IOApp
import cats.effect.kernel.Resource
import cats.syntax.all.*
import edomata.backend.EventMigration
import edomata.backend.PGNaming
import edomata.skunk.*
import io.circe.Codec as JsonCodec
import io.circe.syntax.*
import natchez.Trace.Implicits.noop
import skunk.Session

// ===========================================================================
// EVENT MIGRATION EXAMPLE
//
// This example demonstrates how to evolve event schemas over time using
// Edomata's automatic migration system. Events stored in the journal are
// rewritten in-place, like Flyway for event payloads.
//
// Scenario: A product catalog where:
//   V1 → V2: PriceUpdated gains a `currency` field (defaulting to "USD")
//   V2 → V3: Created gains a `description` field (defaulting to "")
// ===========================================================================

// ---------------------------------------------------------------------------
// Section 1: EVENT VERSIONS
//
// Old event types are kept as sealed enums so the compiler can verify that
// every migration handles ALL cases. Once the migration has run in all
// environments, old types can be deleted.
// ---------------------------------------------------------------------------

/** V1: Original events — the format initially stored in the journal. */
object V1 {
  enum Event derives JsonCodec.AsObject:
    case Created(name: String, priceCents: Long)
    case PriceUpdated(priceCents: Long)
    case Archived
}

/** V2: PriceUpdated now includes currency. */
object V2 {
  enum Event derives JsonCodec.AsObject:
    case Created(name: String, priceCents: Long)
    case PriceUpdated(priceCents: Long, currency: String)
    case Archived
}

/** V3 (current): Created now includes description. This is the type used by the
  * running application — all journal entries will be in this format after
  * migrations run.
  */
object V3 {
  enum Event derives JsonCodec.AsObject:
    case Created(name: String, priceCents: Long, description: String)
    case PriceUpdated(priceCents: Long, currency: String)
    case Archived
}

// ---------------------------------------------------------------------------
// Section 2: MIGRATION DEFINITIONS
//
// Each migration uses the typed factory EventMigration[A, B](...) which
// ensures compile-time exhaustivity. If a case is added to V1.Event and
// not handled in the transform function, the code will NOT compile.
// ---------------------------------------------------------------------------

/** Migration 001: Add currency to PriceUpdated, defaulting to "USD". */
val v1ToV2 =
  EventMigration[V1.Event, V2.Event]("001", "Add currency to PriceUpdated")(
    decode = io.circe.jawn.decode[V1.Event](_).leftMap(_.getMessage),
    transform = {
      // All V1.Event cases MUST be handled here — the compiler enforces this.
      // Try commenting out one case and watch the compilation fail.
      case V1.Event.Created(name, price) => V2.Event.Created(name, price)
      case V1.Event.PriceUpdated(price)  =>
        V2.Event.PriceUpdated(price, currency = "USD")
      case V1.Event.Archived => V2.Event.Archived
    },
    encode = _.asJson.noSpaces
  )

/** Migration 002: Add description to Created, defaulting to empty string. */
val v2ToV3 =
  EventMigration[V2.Event, V3.Event]("002", "Add description to Created")(
    decode = io.circe.jawn.decode[V2.Event](_).leftMap(_.getMessage),
    transform = {
      case V2.Event.Created(name, price) =>
        V3.Event.Created(name, price, description = "")
      case V2.Event.PriceUpdated(price, currency) =>
        V3.Event.PriceUpdated(price, currency)
      case V2.Event.Archived => V3.Event.Archived
    },
    encode = _.asJson.noSpaces
  )

/** All migrations in order. On startup, only pending ones will run. */
val allMigrations = List(v1ToV2, v2ToV3)

// ---------------------------------------------------------------------------
// Section 3: APPLICATION STARTUP
//
// Migrations run automatically before the backend is built. Already-applied
// migrations are skipped (tracked in the `migrations` table). Snapshots are
// invalidated after each migration so the backend rebuilds state from the
// newly-formatted journal.
// ---------------------------------------------------------------------------

object MigrationApp extends IOApp.Simple {
  val naming = PGNaming.prefixed("products")

  val pool: Resource[IO, Resource[IO, Session[IO]]] = Session
    .pooled[IO](
      host = "localhost",
      port = 5432,
      user = "postgres",
      database = "postgres",
      password = Some("postgres"),
      max = 10
    )

  def run: IO[Unit] = pool.use { sessionPool =>
    for {
      // Run migrations — idempotent, safe to call on every startup
      result <- SkunkMigrations.run(naming, sessionPool, allMigrations)
      _ <- IO.println(
        s"Migrations applied: ${result.applied}, skipped: ${result.skipped}"
      )

      // Now build backend with V3 codec only — the journal is guaranteed
      // to contain V3 events after migrations complete.
      // driver <- SkunkDriver.from(naming, sessionPool)
      // backend <- Backend.builder(ProductService).use(driver).build
      // ...
      _ <- IO.println("Backend ready with V3 event format")
    } yield ()
  }
}
