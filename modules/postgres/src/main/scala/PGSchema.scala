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

/** Generates DDL SQL statements for edomata tables.
  *
  * Use this to produce migration scripts (e.g. for Flyway) instead of relying
  * on the automatic table creation performed by the drivers.
  *
  * {{{
  * val ddl = PGSchema.eventsourcing(PGNaming.prefixed("auth"))
  * ddl.foreach(println) // print each DDL statement
  * }}}
  */
object PGSchema {

  /** Generates DDL for event-sourcing tables: journal, outbox, commands, and
    * snapshots.
    *
    * @param naming
    *   the naming strategy (schema or prefixed)
    * @param eventType
    *   PostgreSQL type for event payloads (default: `"jsonb"`)
    * @param notificationType
    *   PostgreSQL type for notification payloads (default: `"jsonb"`)
    * @param snapshotType
    *   PostgreSQL type for snapshot state (default: `"jsonb"`)
    */
  def eventsourcing(
      naming: PGNaming,
      eventType: String = "jsonb",
      notificationType: String = "jsonb",
      snapshotType: String = "jsonb"
  ): List[String] =
    schemaStatement(naming) ++
      journalStatements(naming, eventType) ++
      outboxStatements(naming, notificationType) ++
      commandsStatements(naming) ++
      snapshotsStatements(naming, snapshotType) ++
      migrationsStatements(naming)

  /** Generates DDL for CQRS tables: states, outbox, and commands.
    *
    * @param naming
    *   the naming strategy (schema or prefixed)
    * @param stateType
    *   PostgreSQL type for state payloads (default: `"jsonb"`)
    * @param notificationType
    *   PostgreSQL type for notification payloads (default: `"jsonb"`)
    */
  def cqrs(
      naming: PGNaming,
      stateType: String = "jsonb",
      notificationType: String = "jsonb"
  ): List[String] =
    schemaStatement(naming) ++
      statesStatements(naming, stateType) ++
      outboxStatements(naming, notificationType) ++
      commandsStatements(naming)

  private def schemaStatement(naming: PGNaming): List[String] =
    if naming.needsSchemaSetup then
      List(s"""CREATE SCHEMA IF NOT EXISTS "${naming.namespace}";""")
    else Nil

  private def journalStatements(
      naming: PGNaming,
      payloadType: String
  ): List[String] = {
    val t = naming.table("journal")
    val pk = naming.constraint("journal_pk")
    val un = naming.constraint("journal_un")
    val seqnrIdx = naming.index("journal_seqnr_idx")
    val streamIdx = naming.index("journal_stream_idx")
    List(
      s"""CREATE TABLE IF NOT EXISTS $t (
         |  id uuid NOT NULL,
         |  "time" timestamptz NOT NULL,
         |  seqnr bigserial NOT NULL,
         |  "version" int8 NOT NULL,
         |  stream text NOT NULL,
         |  payload $payloadType NOT NULL,
         |  CONSTRAINT $pk PRIMARY KEY (id),
         |  CONSTRAINT $un UNIQUE (stream, version)
         |);""".stripMargin,
      s"CREATE INDEX IF NOT EXISTS $seqnrIdx ON $t USING btree (seqnr);",
      s"CREATE INDEX IF NOT EXISTS $streamIdx ON $t USING btree (stream, version);"
    )
  }

  private def outboxStatements(
      naming: PGNaming,
      payloadType: String
  ): List[String] = {
    val t = naming.table("outbox")
    val pk = naming.constraint("outbox_pk")
    List(
      s"""CREATE TABLE IF NOT EXISTS $t (
         |  seqnr bigserial NOT NULL,
         |  stream text NOT NULL,
         |  correlation text NULL,
         |  causation text NULL,
         |  payload $payloadType NOT NULL,
         |  created timestamptz NOT NULL,
         |  published timestamptz NULL,
         |  CONSTRAINT $pk PRIMARY KEY (seqnr)
         |);""".stripMargin
    )
  }

  private def commandsStatements(naming: PGNaming): List[String] = {
    val t = naming.table("commands")
    val pk = naming.constraint("commands_pk")
    List(
      s"""CREATE TABLE IF NOT EXISTS $t (
         |  id text NOT NULL,
         |  "time" timestamptz NOT NULL,
         |  address text NOT NULL,
         |  CONSTRAINT $pk PRIMARY KEY (id)
         |);""".stripMargin
    )
  }

  private def snapshotsStatements(
      naming: PGNaming,
      payloadType: String
  ): List[String] = {
    val t = naming.table("snapshots")
    val pk = naming.constraint("snapshots_pk")
    List(
      s"""CREATE TABLE IF NOT EXISTS $t (
         |  id text NOT NULL,
         |  "version" int8 NOT NULL,
         |  state $payloadType NOT NULL,
         |  CONSTRAINT $pk PRIMARY KEY (id)
         |);""".stripMargin
    )
  }

  private def migrationsStatements(naming: PGNaming): List[String] = {
    val t = naming.table("migrations")
    val pk = naming.constraint("migrations_pk")
    List(
      s"""CREATE TABLE IF NOT EXISTS $t (
         |  "version" text NOT NULL,
         |  description text NOT NULL,
         |  applied_at timestamptz NOT NULL DEFAULT now(),
         |  CONSTRAINT $pk PRIMARY KEY ("version")
         |);""".stripMargin
    )
  }

  private def statesStatements(
      naming: PGNaming,
      payloadType: String
  ): List[String] = {
    val t = naming.table("states")
    val pk = naming.constraint("states_pk")
    List(
      s"""CREATE TABLE IF NOT EXISTS $t (
         |  id text NOT NULL,
         |  "version" int8 NOT NULL,
         |  state $payloadType NOT NULL,
         |  CONSTRAINT $pk PRIMARY KEY (id)
         |);""".stripMargin
    )
  }
}
