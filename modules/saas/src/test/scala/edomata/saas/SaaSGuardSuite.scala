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

package edomata.saas

import munit.FunSuite

class SaaSGuardSuite extends FunSuite {

  private val tenantA = TenantId("tenant-a")
  private val tenantB = TenantId("tenant-b")
  private val userA = UserId("user-a")

  private val callerA = CallerIdentity(tenantA, userA, Set("read", "write"))
  private val callerB = CallerIdentity(tenantB, userA, Set("read", "write"))
  private val callerAdmin =
    CallerIdentity(tenantA, userA, Set("read", "write", "admin"))
  private val callerReadOnly = CallerIdentity(tenantA, userA, Set("read"))

  private val activeState =
    CrudState.Active(tenantA, userA, "some data")
  private val deletedState =
    CrudState.Deleted[String](tenantA, userA)

  // --- checkTenant ---

  test("checkTenant: Create action always passes regardless of state") {
    assertEquals(
      SaaSGuard.checkTenant(CrudState.NonExistent, callerA, CrudAction.Create),
      Right(())
    )
    assertEquals(
      SaaSGuard.checkTenant(activeState, callerA, CrudAction.Create),
      Right(())
    )
  }

  test("checkTenant: same tenant passes for Update") {
    assertEquals(
      SaaSGuard.checkTenant(activeState, callerA, CrudAction.Update),
      Right(())
    )
  }

  test("checkTenant: same tenant passes for Delete") {
    assertEquals(
      SaaSGuard.checkTenant(activeState, callerA, CrudAction.Delete),
      Right(())
    )
  }

  test("checkTenant: same tenant passes for Read") {
    assertEquals(
      SaaSGuard.checkTenant(activeState, callerA, CrudAction.Read),
      Right(())
    )
  }

  test("checkTenant: different tenant is rejected for Update") {
    assert(
      SaaSGuard.checkTenant(activeState, callerB, CrudAction.Update).isLeft
    )
  }

  test("checkTenant: different tenant is rejected for Delete") {
    assert(
      SaaSGuard.checkTenant(activeState, callerB, CrudAction.Delete).isLeft
    )
  }

  test("checkTenant: different tenant is rejected for Read") {
    assert(
      SaaSGuard.checkTenant(activeState, callerB, CrudAction.Read).isLeft
    )
  }

  test("checkTenant: NonExistent state is rejected for non-Create actions") {
    assert(
      SaaSGuard
        .checkTenant(CrudState.NonExistent, callerA, CrudAction.Update)
        .isLeft
    )
    assert(
      SaaSGuard
        .checkTenant(CrudState.NonExistent, callerA, CrudAction.Delete)
        .isLeft
    )
    assert(
      SaaSGuard
        .checkTenant(CrudState.NonExistent, callerA, CrudAction.Read)
        .isLeft
    )
  }

  test("checkTenant: Deleted state with same tenant passes") {
    assertEquals(
      SaaSGuard.checkTenant(deletedState, callerA, CrudAction.Read),
      Right(())
    )
  }

  test("checkTenant: Deleted state with different tenant is rejected") {
    assert(
      SaaSGuard.checkTenant(deletedState, callerB, CrudAction.Read).isLeft
    )
  }

  // --- checkRoles ---

  test("checkRoles: caller with all required roles passes") {
    assertEquals(
      SaaSGuard.checkRoles(callerA, Set("read", "write")),
      Right(())
    )
  }

  test("checkRoles: caller with superset of required roles passes") {
    assertEquals(
      SaaSGuard.checkRoles(callerAdmin, Set("read", "write")),
      Right(())
    )
  }

  test("checkRoles: empty required roles always passes") {
    assertEquals(
      SaaSGuard.checkRoles(callerReadOnly, Set.empty),
      Right(())
    )
  }

  test("checkRoles: caller missing roles is rejected") {
    val result = SaaSGuard.checkRoles(callerReadOnly, Set("read", "write"))
    assert(result.isLeft)
    assert(result.left.exists(_.contains("write")))
  }

  test("checkRoles: caller with no matching roles is rejected") {
    val result = SaaSGuard.checkRoles(callerReadOnly, Set("admin"))
    assert(result.isLeft)
    assert(result.left.exists(_.contains("admin")))
  }
}
