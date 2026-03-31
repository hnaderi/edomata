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

  // RoleBasedPolicy for testing
  given AuthPolicy[CallerIdentity] = RoleBasedPolicy {
    case CrudAction.Create => Set("write")
    case CrudAction.Read   => Set("read")
    case CrudAction.Update => Set("write")
    case CrudAction.Delete => Set("admin")
  }

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

  // --- checkAuthorization ---

  test("checkAuthorization: caller with required roles passes") {
    assertEquals(
      SaaSGuard.checkAuthorization(callerA, CrudAction.Update),
      Right(())
    )
  }

  test("checkAuthorization: caller with superset of required roles passes") {
    assertEquals(
      SaaSGuard.checkAuthorization(callerAdmin, CrudAction.Update),
      Right(())
    )
  }

  test("checkAuthorization: caller missing roles is rejected") {
    val result = SaaSGuard.checkAuthorization(callerReadOnly, CrudAction.Update)
    assert(result.isLeft)
    assert(result.left.exists(_.contains("write")))
  }

  test("checkAuthorization: caller with no matching roles is rejected") {
    val result =
      SaaSGuard.checkAuthorization(callerReadOnly, CrudAction.Delete)
    assert(result.isLeft)
    assert(result.left.exists(_.contains("admin")))
  }

  // --- Custom AuthPolicy ---

  case class ApiKeyAuth(key: String, tenant: TenantId, isAdmin: Boolean)

  given AuthPolicy[ApiKeyAuth] = new AuthPolicy[ApiKeyAuth]:
    def tenantId(auth: ApiKeyAuth): TenantId = auth.tenant
    def authorize(auth: ApiKeyAuth, action: CrudAction): Either[String, Unit] =
      if auth.isAdmin then Right(())
      else if action == CrudAction.Delete then Left("Admin only")
      else Right(())

  test("custom AuthPolicy: tenant check works with custom type") {
    val apiKey = ApiKeyAuth("key-1", tenantA, isAdmin = false)
    assertEquals(
      SaaSGuard.checkTenant(activeState, apiKey, CrudAction.Read),
      Right(())
    )
  }

  test("custom AuthPolicy: authorization delegates to custom policy") {
    val regularKey = ApiKeyAuth("key-1", tenantA, isAdmin = false)
    val adminKey = ApiKeyAuth("key-2", tenantA, isAdmin = true)

    assert(SaaSGuard.checkAuthorization(regularKey, CrudAction.Delete).isLeft)
    assertEquals(
      SaaSGuard.checkAuthorization(adminKey, CrudAction.Delete),
      Right(())
    )
    assertEquals(
      SaaSGuard.checkAuthorization(regularKey, CrudAction.Read),
      Right(())
    )
  }
}
