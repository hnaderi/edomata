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
