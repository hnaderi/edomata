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

package edomata.backend

import munit.FunSuite

class PGNamespaceSuite extends FunSuite {
  test("Macro constructor") {
    PGNamespace("a")
    assert(
      compileErrors("PGNamespace(\"\")").contains("Name: \"\" does not match")
    )
  }
  test("Length limit") {
    assertEquals(
      PGNamespace.fromString("a" * 64),
      Left("Name is too long: 64 (max allowed is 63)")
    )
  }

  test("PGNaming.Schema produces schema-qualified table names") {
    val naming = PGNaming.schema(PGNamespace("auth"))
    assertEquals(naming.table("journal"), """"auth".journal""")
    assertEquals(naming.table("outbox"), """"auth".outbox""")
    assertEquals(naming.table("snapshots"), """"auth".snapshots""")
    assertEquals(naming.table("commands"), """"auth".commands""")
    assertEquals(naming.table("states"), """"auth".states""")
  }

  test("PGNaming.Schema does not prefix constraint or index names") {
    val naming = PGNaming.schema(PGNamespace("auth"))
    assertEquals(naming.constraint("journal_pk"), "journal_pk")
    assertEquals(naming.index("journal_seqnr_idx"), "journal_seqnr_idx")
  }

  test("PGNaming.Schema needs schema setup") {
    assert(PGNaming.schema(PGNamespace("auth")).needsSchemaSetup)
  }

  test("PGNaming.Prefixed produces prefixed table names") {
    val naming = PGNaming.prefixed(PGNamespace("auth"))
    assertEquals(naming.table("journal"), "auth_journal")
    assertEquals(naming.table("outbox"), "auth_outbox")
    assertEquals(naming.table("snapshots"), "auth_snapshots")
    assertEquals(naming.table("commands"), "auth_commands")
    assertEquals(naming.table("states"), "auth_states")
  }

  test("PGNaming.Prefixed prefixes constraint and index names") {
    val naming = PGNaming.prefixed(PGNamespace("auth"))
    assertEquals(naming.constraint("journal_pk"), "auth_journal_pk")
    assertEquals(naming.index("journal_seqnr_idx"), "auth_journal_seqnr_idx")
  }

  test("PGNaming.Prefixed does not need schema setup") {
    assert(!PGNaming.prefixed(PGNamespace("auth")).needsSchemaSetup)
  }

  test("PGNamespace.prefixed convenience") {
    val naming = PGNamespace.prefixed("auth")
    assertEquals(naming.table("journal"), "auth_journal")
    assert(!naming.needsSchemaSetup)
  }

  test("PGNaming inline constructors") {
    val schema = PGNaming.schema("auth")
    assertEquals(schema.table("journal"), """"auth".journal""")
    val prefixed = PGNaming.prefixed("auth")
    assertEquals(prefixed.table("journal"), "auth_journal")
  }
}

class PGSchemaSuite extends FunSuite {
  test("eventsourcing DDL with schema mode includes CREATE SCHEMA") {
    val naming = PGNaming.schema(PGNamespace("auth"))
    val ddl = PGSchema.eventsourcing(naming)
    assert(ddl.head.contains("""CREATE SCHEMA IF NOT EXISTS "auth""""))
  }

  test("eventsourcing DDL with prefix mode omits CREATE SCHEMA") {
    val naming = PGNaming.prefixed(PGNamespace("auth"))
    val ddl = PGSchema.eventsourcing(naming)
    assert(ddl.forall(!_.contains("CREATE SCHEMA")))
  }

  test(
    "eventsourcing DDL creates journal, outbox, commands, snapshots tables"
  ) {
    val naming = PGNaming.prefixed(PGNamespace("myapp"))
    val ddl = PGSchema.eventsourcing(naming)
    val all = ddl.mkString("\n")
    assert(all.contains("CREATE TABLE IF NOT EXISTS myapp_journal"))
    assert(all.contains("CREATE TABLE IF NOT EXISTS myapp_outbox"))
    assert(all.contains("CREATE TABLE IF NOT EXISTS myapp_commands"))
    assert(all.contains("CREATE TABLE IF NOT EXISTS myapp_snapshots"))
  }

  test("eventsourcing DDL creates indexes for journal") {
    val naming = PGNaming.prefixed(PGNamespace("myapp"))
    val ddl = PGSchema.eventsourcing(naming)
    val all = ddl.mkString("\n")
    assert(all.contains("CREATE INDEX IF NOT EXISTS myapp_journal_seqnr_idx"))
    assert(all.contains("CREATE INDEX IF NOT EXISTS myapp_journal_stream_idx"))
  }

  test("eventsourcing DDL uses prefixed constraint names") {
    val naming = PGNaming.prefixed(PGNamespace("myapp"))
    val ddl = PGSchema.eventsourcing(naming)
    val all = ddl.mkString("\n")
    assert(all.contains("CONSTRAINT myapp_journal_pk PRIMARY KEY"))
    assert(all.contains("CONSTRAINT myapp_journal_un UNIQUE"))
    assert(all.contains("CONSTRAINT myapp_outbox_pk PRIMARY KEY"))
    assert(all.contains("CONSTRAINT myapp_commands_pk PRIMARY KEY"))
    assert(all.contains("CONSTRAINT myapp_snapshots_pk PRIMARY KEY"))
  }

  test("eventsourcing DDL uses schema-qualified table names in schema mode") {
    val naming = PGNaming.schema(PGNamespace("auth"))
    val ddl = PGSchema.eventsourcing(naming)
    val all = ddl.mkString("\n")
    assert(all.contains(""""auth".journal"""))
    assert(all.contains(""""auth".outbox"""))
    assert(all.contains(""""auth".commands"""))
    assert(all.contains(""""auth".snapshots"""))
  }

  test("eventsourcing DDL uses custom payload types") {
    val naming = PGNaming.prefixed(PGNamespace("myapp"))
    val ddl = PGSchema.eventsourcing(
      naming,
      eventType = "bytea",
      notificationType = "json",
      snapshotType = "jsonb"
    )
    val all = ddl.mkString("\n")
    // journal uses eventType
    assert(
      all.contains("myapp_journal") && all.contains("payload bytea NOT NULL")
    )
    // outbox uses notificationType
    assert(
      all.contains("myapp_outbox") && all.contains("payload json NOT NULL")
    )
    // snapshots uses snapshotType
    assert(
      all.contains("myapp_snapshots") && all.contains("state jsonb NOT NULL")
    )
  }

  test("cqrs DDL creates states, outbox, commands tables") {
    val naming = PGNaming.prefixed(PGNamespace("myapp"))
    val ddl = PGSchema.cqrs(naming)
    val all = ddl.mkString("\n")
    assert(all.contains("CREATE TABLE IF NOT EXISTS myapp_states"))
    assert(all.contains("CREATE TABLE IF NOT EXISTS myapp_outbox"))
    assert(all.contains("CREATE TABLE IF NOT EXISTS myapp_commands"))
    assert(!all.contains("myapp_journal"))
    assert(!all.contains("myapp_snapshots"))
  }

  test("cqrs DDL with schema mode includes CREATE SCHEMA") {
    val naming = PGNaming.schema(PGNamespace("auth"))
    val ddl = PGSchema.cqrs(naming)
    assert(ddl.head.contains("""CREATE SCHEMA IF NOT EXISTS "auth""""))
  }

  test("DDL statements are valid standalone SQL") {
    val naming = PGNaming.prefixed(PGNamespace("test"))
    val ddl = PGSchema.eventsourcing(naming)
    ddl.foreach { stmt =>
      assert(
        stmt.endsWith(";"),
        s"Statement does not end with semicolon: $stmt"
      )
    }
  }
}
