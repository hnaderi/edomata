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

package edomata.saas

import edomata.backend.PGNaming
import edomata.backend.PGNamespace
import munit.FunSuite

class TenantExtractorSuite extends FunSuite {

  test("extracts tenant and owner from Active state") {
    val tid = TenantId("t1")
    val oid = UserId("u1")
    val state = CrudState.Active(tid, oid, "data")
    assertEquals(
      TenantExtractor[CrudState[String]].extract(state),
      Some((tid, oid))
    )
  }

  test("extracts tenant and owner from Deleted state") {
    val tid = TenantId("t1")
    val oid = UserId("u1")
    val state: CrudState[String] = CrudState.Deleted(tid, oid)
    assertEquals(
      TenantExtractor[CrudState[String]].extract(state),
      Some((tid, oid))
    )
  }

  test("returns None for NonExistent state") {
    val state: CrudState[String] = CrudState.NonExistent
    assertEquals(TenantExtractor[CrudState[String]].extract(state), None)
  }
}

class SaaSPGSchemaSuite extends FunSuite {

  test("cqrs DDL with prefix mode contains tenant_id and owner_id in states") {
    val naming = PGNaming.prefixed(PGNamespace("catalog"))
    val ddl = SaaSPGSchema.cqrs(naming)
    val all = ddl.mkString("\n")
    assert(all.contains("catalog_states"))
    assert(all.contains("tenant_id text NOT NULL"))
    assert(all.contains("owner_id text NOT NULL"))
  }

  test("cqrs DDL creates states with tenant indexes") {
    val naming = PGNaming.prefixed(PGNamespace("catalog"))
    val ddl = SaaSPGSchema.cqrs(naming)
    val all = ddl.mkString("\n")
    assert(all.contains("catalog_states_tenant_idx"))
    assert(all.contains("catalog_states_tenant_owner_idx"))
  }

  test("cqrs DDL creates outbox with tenant_id column and index") {
    val naming = PGNaming.prefixed(PGNamespace("catalog"))
    val ddl = SaaSPGSchema.cqrs(naming)
    val all = ddl.mkString("\n")
    assert(all.contains("catalog_outbox"))
    assert(all.contains("catalog_outbox_tenant_idx"))
    // outbox should have tenant_id
    val outboxDdl = ddl
      .find(s => s.contains("catalog_outbox") && s.contains("CREATE TABLE"))
      .get
    assert(outboxDdl.contains("tenant_id text NOT NULL"))
  }

  test("cqrs DDL creates commands table without tenant_id") {
    val naming = PGNaming.prefixed(PGNamespace("catalog"))
    val ddl = SaaSPGSchema.cqrs(naming)
    val cmdDdl = ddl
      .find(s => s.contains("catalog_commands") && s.contains("CREATE TABLE"))
      .get
    assert(!cmdDdl.contains("tenant_id"))
  }

  test("cqrs DDL with schema mode includes CREATE SCHEMA") {
    val naming = PGNaming.schema(PGNamespace("catalog"))
    val ddl = SaaSPGSchema.cqrs(naming)
    assert(ddl.head.contains("""CREATE SCHEMA IF NOT EXISTS "catalog""""))
  }

  test("cqrs DDL with prefix mode omits CREATE SCHEMA") {
    val naming = PGNaming.prefixed(PGNamespace("catalog"))
    val ddl = SaaSPGSchema.cqrs(naming)
    assert(ddl.forall(!_.contains("CREATE SCHEMA")))
  }

  test("cqrs DDL respects custom payload types") {
    val naming = PGNaming.prefixed(PGNamespace("app"))
    val ddl =
      SaaSPGSchema.cqrs(naming, stateType = "bytea", notificationType = "json")
    val all = ddl.mkString("\n")
    assert(all.contains("state bytea NOT NULL"))
    assert(all.contains("payload json NOT NULL"))
  }

  test("cqrs DDL without RLS produces no ALTER TABLE or POLICY") {
    val naming = PGNaming.prefixed(PGNamespace("app"))
    val ddl = SaaSPGSchema.cqrs(naming, rls = None)
    val all = ddl.mkString("\n")
    assert(!all.contains("ROW LEVEL SECURITY"))
    assert(!all.contains("CREATE POLICY"))
  }

  test("cqrs DDL with RLS produces ALTER TABLE, POLICY, and GRANT") {
    val naming = PGNaming.prefixed(PGNamespace("app"))
    val rls = SaaSPGSchema.RLSConfig("app_user", "app.tenant_id")
    val ddl = SaaSPGSchema.cqrs(naming, rls = Some(rls))
    val all = ddl.mkString("\n")
    assert(all.contains("ALTER TABLE app_states ENABLE ROW LEVEL SECURITY"))
    assert(all.contains("CREATE POLICY app_states_tenant_policy ON app_states"))
    assert(all.contains("current_setting('app.tenant_id')"))
    assert(
      all.contains("GRANT SELECT, INSERT, UPDATE ON app_states TO app_user")
    )
    assert(all.contains("ALTER TABLE app_outbox ENABLE ROW LEVEL SECURITY"))
  }

  test("all DDL statements end with semicolons") {
    val naming = PGNaming.prefixed(PGNamespace("test"))
    val ddl = SaaSPGSchema.cqrs(naming)
    ddl.foreach { stmt =>
      assert(
        stmt.endsWith(";"),
        s"Statement does not end with semicolon: $stmt"
      )
    }
  }
}
