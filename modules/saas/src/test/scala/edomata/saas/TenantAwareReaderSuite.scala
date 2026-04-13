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

import cats.Id
import munit.FunSuite

class TenantAwareReaderSuite extends FunSuite {

  private val tenantA = TenantId("tenant-a")
  private val tenantB = TenantId("tenant-b")
  private val userA = UserId("user-a")

  private val callerA =
    CallerIdentity(tenantA, userA, Set("read"))
  private val callerB =
    CallerIdentity(tenantB, userA, Set("read"))

  given AuthPolicy[CallerIdentity] = RoleBasedPolicy(_ => Set.empty)

  // --- TenantScopedQuery ---

  test("TenantScopedQuery: passes caller's tenantId to the query function") {
    var capturedTenantId: TenantId = TenantId("")
    val query = TenantScopedQuery[Id, CallerIdentity, String, Unit] {
      (tid, _) =>
        capturedTenantId = tid
        List("item1", "item2")
    }

    val result = query.query(callerA, ())
    assertEquals(capturedTenantId, tenantA)
    assertEquals(result, List("item1", "item2"))
  }

  test("TenantScopedQuery: different callers get different tenant filters") {
    val query = TenantScopedQuery[Id, CallerIdentity, String, Unit] {
      (tid, _) =>
        List(s"from-${tid.value}")
    }

    assertEquals(query.query(callerA, ()), List("from-tenant-a"))
    assertEquals(query.query(callerB, ()), List("from-tenant-b"))
  }

  test("TenantScopedQuery: passes query parameter through") {
    var capturedQuery: String = ""
    val query = TenantScopedQuery[Id, CallerIdentity, String, String] {
      (_, q) =>
        capturedQuery = q
        List.empty
    }

    query.query(callerA, "search-term")
    assertEquals(capturedQuery, "search-term")
  }

  test("TenantScopedQuery: returns empty list when no results") {
    val query = TenantScopedQuery[Id, CallerIdentity, String, Unit] { (_, _) =>
      List.empty
    }
    assertEquals(query.query(callerA, ()), List.empty[String])
  }

  // --- UnsafeCrossTenantQuery ---

  test("UnsafeCrossTenantQuery: does not require CallerIdentity") {
    val query = UnsafeCrossTenantQuery[Id, String, Unit] { _ =>
      List("all-tenants-item1", "all-tenants-item2")
    }

    val result = query.query(())
    assertEquals(result, List("all-tenants-item1", "all-tenants-item2"))
  }

  test("UnsafeCrossTenantQuery: passes query parameter through") {
    var capturedQuery: Int = 0
    val query = UnsafeCrossTenantQuery[Id, String, Int] { q =>
      capturedQuery = q
      List.empty
    }

    query.query(42)
    assertEquals(capturedQuery, 42)
  }

  test("UnsafeCrossTenantQuery: returns empty list when no results") {
    val query = UnsafeCrossTenantQuery[Id, String, Unit] { _ =>
      List.empty
    }
    assertEquals(query.query(()), List.empty[String])
  }

  // --- TenantAwareReader (trait usage) ---

  test("TenantAwareReader: can be implemented for single entity reads") {
    val store = Map(
      ("tenant-a", "e1") -> "data-1",
      ("tenant-b", "e2") -> "data-2"
    )

    val reader = new TenantAwareReader[Id, CallerIdentity, String] {
      def get(auth: CallerIdentity, entityId: String): Option[String] =
        store.get((auth.tenantId.value, entityId))
    }

    assertEquals(reader.get(callerA, "e1"), Some("data-1"))
    assertEquals(reader.get(callerA, "e2"), None) // wrong tenant
    assertEquals(reader.get(callerB, "e2"), Some("data-2"))
    assertEquals(reader.get(callerB, "e1"), None) // wrong tenant
  }
}
