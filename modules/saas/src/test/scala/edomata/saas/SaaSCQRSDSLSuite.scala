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
import cats.data.*
import edomata.core.*
import munit.FunSuite

class SaaSCQRSDSLSuite extends FunSuite {

  private val tenantA = TenantId("tenant-a")
  private val tenantB = TenantId("tenant-b")
  private val userA = UserId("user-a")

  private val callerWriter =
    CallerIdentity(tenantA, userA, Set("read", "write"))
  private val callerReadOnly =
    CallerIdentity(tenantA, userA, Set("read"))
  private val callerOtherTenant =
    CallerIdentity(tenantB, userA, Set("read", "write"))

  // --- Test DSL setup ---

  type TestState = CrudState[String]
  type TestNotification = String

  given AuthPolicy[CallerIdentity] = RoleBasedPolicy {
    case CrudAction.Create => Set("write")
    case CrudAction.Read   => Set("read")
    case CrudAction.Update => Set("write")
    case CrudAction.Delete => Set("admin")
  }

  private val dsl =
    SaaSCQRSDomainDSL[CallerIdentity, String, String, String, TestNotification](
      mkRejection = identity
    )

  private val activeState: TestState =
    CrudState.Active(tenantA, userA, "hello")

  private def mkCmd(
      caller: CallerIdentity,
      payload: String
  ): CommandMessage[SaaSCommand[CallerIdentity, String]] =
    CommandMessage(
      "cmd-1",
      java.time.Instant.now(),
      "entity-1",
      SaaSCommand(caller, payload): SaaSCommand[CallerIdentity, String]
    )

  private def run(
      app: dsl.App[Id, Unit],
      caller: CallerIdentity,
      state: TestState,
      payload: String = "test"
  ): ResponseE[String, TestNotification, (TestState, Unit)] =
    app.run(mkCmd(caller, payload), state)

  // --- guardedRouter tests ---

  private val guardedApp: dsl.App[Id, Unit] = dsl.guardedRouter {
    case "create" =>
      (
        CrudAction.Create,
        dsl.set(CrudState.Active(tenantA, userA, "created"))
      )
    case "update" =>
      (
        CrudAction.Update,
        dsl.set(CrudState.Active(tenantA, userA, "updated"))
      )
    case "delete" =>
      (CrudAction.Delete, dsl.reject("delete not implemented"))
    case _ =>
      (CrudAction.Read, dsl.unit)
  }

  test("guardedRouter: correct tenant + roles executes business logic") {
    val result = run(guardedApp, callerWriter, activeState, "update")
    assert(result.result.isRight, s"Expected Right, got: ${result.result}")
    result.result match
      case Right((CrudState.Active(_, _, data), ())) =>
        assertEquals(data, "updated")
      case other => fail(s"Unexpected result: $other")
  }

  test("guardedRouter: Create bypasses tenant check on NonExistent state") {
    val result =
      run(guardedApp, callerWriter, CrudState.NonExistent, "create")
    assert(result.result.isRight, s"Expected Right, got: ${result.result}")
  }

  test("guardedRouter: wrong tenant is rejected") {
    val result =
      run(guardedApp, callerOtherTenant, activeState, "update")
    assert(result.result.isLeft, s"Expected Left, got: ${result.result}")
    result.result match
      case Left(errs) =>
        assert(
          errs.exists(_.contains("Tenant mismatch")),
          s"Expected tenant mismatch, got: $errs"
        )
      case _ => fail("Expected rejection")
  }

  test("guardedRouter: missing roles is rejected") {
    val result =
      run(guardedApp, callerReadOnly, activeState, "update")
    assert(result.result.isLeft, s"Expected Left, got: ${result.result}")
    result.result match
      case Left(errs) =>
        assert(
          errs.exists(_.contains("Missing roles")),
          s"Expected missing roles, got: $errs"
        )
      case _ => fail("Expected rejection")
  }

  test("guardedRouter: NonExistent state is rejected for non-Create") {
    val result =
      run(guardedApp, callerWriter, CrudState.NonExistent, "update")
    assert(result.result.isLeft, s"Expected Left, got: ${result.result}")
    result.result match
      case Left(errs) =>
        assert(
          errs.exists(_.contains("Entity not found")),
          s"Expected not found, got: $errs"
        )
      case _ => fail("Expected rejection")
  }

  // --- unsafeUnguardedRouter tests ---

  private val unsafeApp: dsl.App[Id, Unit] = dsl.unsafeUnguardedRouter {
    case "update" =>
      dsl.set(CrudState.Active(tenantA, userA, "admin-updated"))
    case _ => dsl.unit
  }

  test("unsafeUnguardedRouter: wrong tenant is NOT rejected (bypass)") {
    val result =
      run(unsafeApp, callerOtherTenant, activeState, "update")
    assert(result.result.isRight, s"Expected Right, got: ${result.result}")
    result.result match
      case Right((CrudState.Active(_, _, data), ())) =>
        assertEquals(data, "admin-updated")
      case other => fail(s"Unexpected result: $other")
  }

  test("unsafeUnguardedRouter: missing roles is NOT rejected (bypass)") {
    val result =
      run(unsafeApp, callerReadOnly, activeState, "update")
    assert(result.result.isRight, s"Expected Right, got: ${result.result}")
  }

  // --- notifications ---

  private val notifyApp: dsl.App[Id, Unit] = dsl.guardedRouter {
    case "update" =>
      val logic: dsl.App[Id, Unit] = for
        _ <- dsl.set[Id](CrudState.Active(tenantA, userA, "updated"))
        _ <- dsl.publish[Id]("entity-updated")
      yield ()
      (CrudAction.Update, logic)
    case _ => (CrudAction.Read, dsl.unit)
  }

  test("guardedRouter: notifications are emitted on success") {
    val result = run(notifyApp, callerWriter, activeState, "update")
    assert(result.result.isRight)
    assertEquals(result.notifications, Chain("entity-updated"))
  }

  test("guardedRouter: no notifications on guard rejection") {
    val result =
      run(notifyApp, callerOtherTenant, activeState, "update")
    assert(result.result.isLeft)
    assertEquals(result.notifications, Chain.empty[String])
  }
}
