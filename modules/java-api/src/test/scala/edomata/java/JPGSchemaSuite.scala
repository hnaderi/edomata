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

import munit.FunSuite

class JPGSchemaSuite extends FunSuite {

  test("eventsourcing generates DDL statements") {
    val ddl = JPGSchema.eventsourcing("myapp")
    assert(ddl.size() > 0)
    // Should contain journal, outbox, commands, snapshots tables
    val allDDL = ddl.toString
    assert(
      allDDL.contains("myapp_journal"),
      s"Missing journal table in: $allDDL"
    )
    assert(allDDL.contains("myapp_outbox"), s"Missing outbox table in: $allDDL")
    assert(
      allDDL.contains("myapp_commands"),
      s"Missing commands table in: $allDDL"
    )
    assert(
      allDDL.contains("myapp_snapshots"),
      s"Missing snapshots table in: $allDDL"
    )
  }

  test("eventsourcing with custom types") {
    val ddl = JPGSchema.eventsourcing("test", "json", "json", "bytea")
    val allDDL = ddl.toString
    assert(allDDL.contains("json"))
    assert(allDDL.contains("bytea"))
  }

  test("cqrs generates DDL statements") {
    val ddl = JPGSchema.cqrs("myapp")
    assert(ddl.size() > 0)
    val allDDL = ddl.toString
    assert(allDDL.contains("myapp_states"), s"Missing states table in: $allDDL")
    assert(allDDL.contains("myapp_outbox"), s"Missing outbox table in: $allDDL")
    assert(
      allDDL.contains("myapp_commands"),
      s"Missing commands table in: $allDDL"
    )
  }

  test("eventsourcingWithSchema generates schema DDL") {
    val ddl = JPGSchema.eventsourcingWithSchema("auth")
    val allDDL = ddl.toString
    assert(
      allDDL.contains("CREATE SCHEMA"),
      s"Missing CREATE SCHEMA in: $allDDL"
    )
    assert(
      allDDL.contains(""""auth".journal"""),
      s"Missing schema-qualified table in: $allDDL"
    )
  }

  test("cqrsWithSchema generates schema DDL") {
    val ddl = JPGSchema.cqrsWithSchema("auth")
    val allDDL = ddl.toString
    assert(allDDL.contains("CREATE SCHEMA"))
  }

  test("invalid namespace throws") {
    intercept[IllegalArgumentException] {
      JPGSchema.eventsourcing("")
    }
  }
}
