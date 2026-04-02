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

package edomata.backend

/** A versioned event migration that transforms journal payloads.
  *
  * Each migration has a unique version string (like "001" or "v2-add-email")
  * and a description. Migrations are applied in list order when passed to
  * `SkunkMigrations.run` or `DoobieMigrations.run`.
  *
  * Already-applied migrations (tracked in the `migrations` table) are skipped
  * automatically, making the process idempotent like Flyway.
  *
  * Use the typed factory to benefit from compile-time exhaustivity checking:
  * {{{
  * val migration = EventMigration[OldEvent, NewEvent]("001", "Add email")(
  *   decode = io.circe.jawn.decode[OldEvent](_).leftMap(_.getMessage),
  *   transform = {
  *     case OldEvent.Created(name) => NewEvent.Created(name, email = "")
  *     // Missing cases cause a compile error when OldEvent is a sealed enum
  *   },
  *   encode = _.asJson.noSpaces
  * )
  * }}}
  */
final case class EventMigration(
    version: String,
    description: String,
    run: String => Either[String, String]
) {

  /** Compose this migration with a subsequent one. The resulting migration has
    * the version and description of `next`.
    */
  def andThen(next: EventMigration): EventMigration =
    EventMigration(
      version = next.version,
      description = next.description,
      run = raw => this.run(raw).flatMap(next.run)
    )
}

object EventMigration {

  /** Build a typed migration with compile-time exhaustivity checking.
    *
    * When `A` is a sealed enum and `transform` uses pattern matching, Scala 3
    * will emit a compile error if any case is missing. This prevents incomplete
    * migrations from being deployed.
    *
    * @tparam A
    *   the old event type (should be a sealed enum)
    * @tparam B
    *   the new event type
    * @param version
    *   unique identifier for this migration (e.g. "001", "v2-add-email")
    * @param description
    *   human-readable description of what this migration does
    * @param decode
    *   deserializes raw JSON string to the old event type
    * @param transform
    *   total function converting old events to new events
    * @param encode
    *   serializes new events to JSON string
    */
  def apply[A, B](version: String, description: String)(
      decode: String => Either[String, A],
      transform: A => B,
      encode: B => String
  ): EventMigration =
    EventMigration(
      version,
      description,
      raw => decode(raw).map(a => encode(transform(a)))
    )
}

/** Result of running migrations. */
final case class MigrationResult(
    applied: List[String],
    skipped: List[String]
)
