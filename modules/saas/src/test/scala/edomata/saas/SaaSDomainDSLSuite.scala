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
import cats.implicits.*
import edomata.core.*
import munit.FunSuite

class SaaSDomainDSLSuite extends FunSuite {

  private val tenantA = TenantId("tenant-a")
  private val tenantB = TenantId("tenant-b")
  private val userA = UserId("user-a")

  private val callerWriter =
    CallerIdentity(tenantA, userA, Set("read", "write"))
  private val callerReadOnly =
    CallerIdentity(tenantA, userA, Set("read"))
  private val callerOtherTenant =
    CallerIdentity(tenantB, userA, Set("read", "write"))

  // --- Test domain for event sourcing ---

  type TestEvent = String
  type TestRejection = String
  type TestNotification = String

  given AuthPolicy[CallerIdentity] = RoleBasedPolicy {
    case CrudAction.Create => Set("write")
    case CrudAction.Read   => Set("read")
    case CrudAction.Update => Set("write")
    case CrudAction.Delete => Set("admin")
  }

  private val dsl =
    SaaSDomainDSL[
      CallerIdentity,
      String,
      String,
      TestEvent,
      TestRejection,
      TestNotification
    ](
      mkRejection = identity
    )

  private val activeState: CrudState[String] =
    CrudState.Active(tenantA, userA, "hello")

  private def mkCtx(
      caller: CallerIdentity,
      state: CrudState[String],
      payload: String = "test"
  ): RequestContext[SaaSCommand[CallerIdentity, String], CrudState[String]] =
    RequestContext(
      CommandMessage(
        "cmd-1",
        java.time.Instant.now(),
        "entity-1",
        SaaSCommand(caller, payload): SaaSCommand[CallerIdentity, String]
      ),
      state
    )

  private def run(
      app: dsl.App[Id, Unit],
      caller: CallerIdentity,
      state: CrudState[String],
      payload: String = "test"
  ): ResponseD[TestRejection, TestEvent, TestNotification, Unit] =
    app.run(mkCtx(caller, state, payload))

  // --- guardedRouter ---

  private val guardedApp: dsl.App[Id, Unit] = dsl.guardedRouter {
    case "create" =>
      (CrudAction.Create, dsl.decide(Decision.accept("created")))
    case "update" =>
      (CrudAction.Update, dsl.decide(Decision.accept("updated")))
    case "delete" =>
      (CrudAction.Delete, dsl.reject("delete not allowed"))
    case _ =>
      (CrudAction.Read, dsl.unit)
  }

  test("guardedRouter: correct tenant + roles executes business logic") {
    val result = run(guardedApp, callerWriter, activeState, "update")
    result.result match
      case Decision.Accepted(evs, _) =>
        assertEquals(evs.toList, List("updated"))
      case other => fail(s"Expected Accepted, got: $other")
  }

  test("guardedRouter: Create bypasses tenant check on NonExistent state") {
    val result = run(guardedApp, callerWriter, CrudState.NonExistent, "create")
    assert(
      result.result.isAccepted,
      s"Expected Accepted, got: ${result.result}"
    )
  }

  test("guardedRouter: wrong tenant is rejected") {
    val result = run(guardedApp, callerOtherTenant, activeState, "update")
    assert(
      result.result.isRejected,
      s"Expected Rejected, got: ${result.result}"
    )
  }

  test("guardedRouter: missing roles is rejected") {
    val result = run(guardedApp, callerReadOnly, activeState, "update")
    assert(
      result.result.isRejected,
      s"Expected Rejected, got: ${result.result}"
    )
  }

  test("guardedRouter: NonExistent state is rejected for non-Create") {
    val result = run(guardedApp, callerWriter, CrudState.NonExistent, "update")
    assert(
      result.result.isRejected,
      s"Expected Rejected, got: ${result.result}"
    )
  }

  // --- unsafeUnguardedRouter ---

  private val unsafeApp: dsl.App[Id, Unit] = dsl.unsafeUnguardedRouter {
    case "update" => dsl.decide(Decision.accept("admin-updated"))
    case _        => dsl.unit
  }

  test("unsafeUnguardedRouter: wrong tenant is NOT rejected (bypass)") {
    val result = run(unsafeApp, callerOtherTenant, activeState, "update")
    result.result match
      case Decision.Accepted(evs, _) =>
        assertEquals(evs.toList, List("admin-updated"))
      case other => fail(s"Expected Accepted, got: $other")
  }

  test("unsafeUnguardedRouter: missing roles is NOT rejected (bypass)") {
    val result = run(unsafeApp, callerReadOnly, activeState, "update")
    result.result match
      case Decision.Accepted(evs, _) =>
        assertEquals(evs.toList, List("admin-updated"))
      case other => fail(s"Expected Accepted, got: $other")
  }

  // --- guarded (single action) ---

  test("guarded: runs guard then logic for matching action") {
    val app: dsl.App[Id, Unit] =
      dsl.guarded(CrudAction.Update)(dsl.decide(Decision.accept("updated")))
    val result = run(app, callerWriter, activeState)
    assert(
      result.result.isAccepted,
      s"Expected Accepted, got: ${result.result}"
    )
  }

  test("guarded: rejects on tenant mismatch") {
    val app: dsl.App[Id, Unit] =
      dsl.guarded(CrudAction.Update)(dsl.decide(Decision.accept("updated")))
    val result = run(app, callerOtherTenant, activeState)
    assert(result.result.isRejected)
  }

  // --- unsafeUnguarded (single action) ---

  test("unsafeUnguarded: executes without guard") {
    val app: dsl.App[Id, Unit] =
      dsl.unsafeUnguarded(dsl.decide(Decision.accept("updated")))
    val result = run(app, callerOtherTenant, activeState)
    assert(
      result.result.isAccepted,
      s"Expected Accepted, got: ${result.result}"
    )
  }

  // --- accessor methods ---

  test("auth: extracts auth context from command") {
    val app = dsl.auth[Id].map(c => assertEquals(c, callerWriter))
    run(app.void, callerWriter, activeState)
  }

  test("command: extracts business command payload") {
    val app = dsl.command[Id].map(c => assertEquals(c, "my-command"))
    run(app.void, callerWriter, activeState, "my-command")
  }

  test("entityState: extracts CrudState") {
    val app = dsl.entityState[Id].map(s => assertEquals(s, activeState))
    run(app.void, callerWriter, activeState)
  }

  test("aggregateId: extracts entity address") {
    val app = dsl.aggregateId[Id].map(id => assertEquals(id, "entity-1"))
    run(app.void, callerWriter, activeState)
  }

  // --- DSL delegation methods ---

  test("pure: lifts a value") {
    val app = dsl.pure[Id, Int](42).map(v => assertEquals(v, 42))
    run(app.void, callerWriter, activeState)
  }

  test("unit: returns unit") {
    val result = run(dsl.unit[Id], callerWriter, activeState)
    result.result match
      case Decision.InDecisive(()) => () // ok
      case other => fail(s"Expected InDecisive(()), got: $other")
  }

  test("reject: produces rejection") {
    val app: dsl.App[Id, Unit] = dsl.reject("bad request")
    val result = run(app, callerWriter, activeState)
    assert(result.result.isRejected)
    result.result.visit(
      errs => assert(errs.toList.contains("bad request")),
      _ => fail("Expected rejection")
    )
  }

  test("decide: applies Decision") {
    val app: dsl.App[Id, Unit] = dsl.decide(Decision.accept("evt")).void
    val result = run(app, callerWriter, activeState)
    result.result match
      case Decision.Accepted(evs, _) =>
        assertEquals(evs.toList, List("evt"))
      case other => fail(s"Expected Accepted, got: $other")
  }

  test("publish: emits notifications") {
    val app: dsl.App[Id, Unit] = dsl.publish("notif-1", "notif-2")
    val result = run(app, callerWriter, activeState)
    assertEquals(result.notifications, Chain("notif-1", "notif-2"))
  }

  test("validate: passes on Valid") {
    val app = dsl
      .validate[Id, Int](Validated.validNec(42))
      .map(v => assertEquals(v, 42))
    run(app.void, callerWriter, activeState)
  }

  test("validate: rejects on Invalid") {
    val app: dsl.App[Id, Unit] =
      dsl.validate[Id, Int](Validated.invalidNec("err")).void
    val result = run(app, callerWriter, activeState)
    assert(result.result.isRejected)
  }

  // --- notifications on guarded rejection ---

  test("guardedRouter: no notifications on guard rejection") {
    val app: dsl.App[Id, Unit] = dsl.guardedRouter { case _ =>
      (
        CrudAction.Update,
        dsl.publish[Id]("should-not-appear")
      )
    }
    val result = run(app, callerOtherTenant, activeState)
    assert(result.result.isRejected)
    assertEquals(result.notifications, Chain.empty[String])
  }

  test("guardedRouter: notifications emitted on success") {
    val app: dsl.App[Id, Unit] = dsl.guardedRouter { case _ =>
      val logic: dsl.App[Id, Unit] = for
        _ <- dsl.decide[Id, Unit](Decision.accept("evt"))
        _ <- dsl.publish[Id]("notif")
      yield ()
      (CrudAction.Update, logic)
    }
    val result = run(app, callerWriter, activeState)
    assert(result.result.isAccepted)
    assertEquals(result.notifications, Chain("notif"))
  }

  // --- Deleted state ---

  test("guardedRouter: Deleted state with same tenant passes guard") {
    val deletedState = CrudState.Deleted[String](tenantA, userA)
    val app: dsl.App[Id, Unit] = dsl.guardedRouter { case _ =>
      (CrudAction.Read, dsl.unit)
    }
    val result = run(app, callerWriter, deletedState)
    result.result match
      case Decision.InDecisive(()) => () // ok
      case other                   => fail(s"Expected InDecisive, got: $other")
  }

  test("guardedRouter: Deleted state with wrong tenant is rejected") {
    val deletedState = CrudState.Deleted[String](tenantA, userA)
    val app: dsl.App[Id, Unit] = dsl.guardedRouter { case _ =>
      (CrudAction.Read, dsl.unit)
    }
    val result = run(app, callerOtherTenant, deletedState)
    assert(result.result.isRejected)
  }
}
