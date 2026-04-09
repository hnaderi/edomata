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

package edomata.java

import edomata.backend.PGNamespace
import edomata.backend.PGNaming
import edomata.backend.PGSchema

import scala.jdk.CollectionConverters.*

/** Java-friendly DDL extraction for Flyway / Liquibase migrations.
  *
  * Each method returns a list of standalone SQL statements (CREATE TABLE,
  * CREATE INDEX, etc.).
  */
object JPGSchema {

  /** Generate DDL for event sourcing tables (journal, outbox, commands,
    * snapshots).
    *
    * @param namespace
    *   the namespace (used as table prefix)
    * @param eventType
    *   PostgreSQL payload type: "json", "jsonb", or "bytea"
    * @param notificationType
    *   PostgreSQL payload type for notifications
    * @param snapshotType
    *   PostgreSQL payload type for snapshots
    */
  def eventsourcing(
      namespace: String,
      eventType: String,
      notificationType: String,
      snapshotType: String
  ): java.util.List[String] =
    PGSchema
      .eventsourcing(
        naming(namespace),
        eventType,
        notificationType,
        snapshotType
      )
      .asJava

  /** Generate DDL for event sourcing tables with default jsonb payload types.
    */
  def eventsourcing(namespace: String): java.util.List[String] =
    PGSchema.eventsourcing(naming(namespace)).asJava

  /** Generate DDL for CQRS tables (states, outbox, commands).
    *
    * @param namespace
    *   the namespace (used as table prefix)
    * @param stateType
    *   PostgreSQL payload type for state
    * @param notificationType
    *   PostgreSQL payload type for notifications
    */
  def cqrs(
      namespace: String,
      stateType: String,
      notificationType: String
  ): java.util.List[String] =
    PGSchema
      .cqrs(naming(namespace), stateType, notificationType)
      .asJava

  /** Generate DDL for CQRS tables with default jsonb payload types. */
  def cqrs(namespace: String): java.util.List[String] =
    PGSchema.cqrs(naming(namespace)).asJava

  /** Generate DDL using schema naming strategy (creates a PostgreSQL schema).
    */
  def eventsourcingWithSchema(
      namespace: String
  ): java.util.List[String] =
    PGSchema
      .eventsourcing(schemaNaming(namespace))
      .asJava

  /** Generate DDL for CQRS using schema naming strategy. */
  def cqrsWithSchema(namespace: String): java.util.List[String] =
    PGSchema.cqrs(schemaNaming(namespace)).asJava

  private def naming(ns: String): PGNaming = {
    val pgns = PGNamespace
      .fromString(ns)
      .fold(
        err => throw new IllegalArgumentException(s"Invalid namespace: $err"),
        identity
      )
    PGNaming.Prefixed(pgns)
  }

  private def schemaNaming(ns: String): PGNaming = {
    val pgns = PGNamespace
      .fromString(ns)
      .fold(
        err => throw new IllegalArgumentException(s"Invalid namespace: $err"),
        identity
      )
    PGNaming.Schema(pgns)
  }
}
