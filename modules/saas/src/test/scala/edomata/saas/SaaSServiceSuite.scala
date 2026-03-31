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

import cats.Id
import cats.data.*
import cats.implicits.*
import edomata.core.*
import munit.FunSuite

class SaaSServiceSuite extends FunSuite {

  private val tenantA = TenantId("tenant-a")
  private val userA = UserId("user-a")

  // ========================================================================
  // CQRS service tests
  // ========================================================================

  object TestCQRSModel extends CQRSModel[CrudState[String], String]:
    def initial: CrudState[String] = CrudState.NonExistent

  import TestCQRSModel.given

  object TestCQRSService
      extends SaaSCQRSService[String, String, String, String](
        rolesFor = _ => Set.empty,
        mkRejection = identity
      ):
    def apply(): App[Id, Unit] = SaaS.guardedRouter {
      case "create" =>
        (
          CrudAction.Create,
          SaaS.set(CrudState.Active(tenantA, userA, "created"))
        )
      case "update" =>
        val logic: App[Id, Unit] = for _ <- SaaS.modifyS[Id] {
            case CrudState.Active(tid, oid, _) =>
              Right(CrudState.Active(tid, oid, "updated"))
            case s => Left(NonEmptyChain.one("not active"))
          }
        yield ()
        (CrudAction.Update, logic)
      case "decideS" =>
        val logic: App[Id, Unit] = for _ <- SaaS.decideS[Id] {
            case CrudState.Active(tid, oid, _) =>
              Right(CrudState.Active(tid, oid, "decided"))
            case s => Left(NonEmptyChain.one("not active"))
          }
        yield ()
        (CrudAction.Update, logic)
      case "eval" =>
        (CrudAction.Read, SaaS.eval[Id, Unit](()))
      case _ => (CrudAction.Read, SaaS.unit)
    }

  test("SaaSCQRSService: domain field is accessible") {
    val _ = TestCQRSService.domain // compiles = accessible
  }

  test("SaaSCQRSService: set changes state") {
    val cmd = CommandMessage(
      "c1",
      java.time.Instant.now(),
      "e1",
      SaaSCommand(CallerIdentity(tenantA, userA, Set.empty), "create")
    )
    val result = TestCQRSService().run(cmd, CrudState.NonExistent)
    result.result match
      case Right((CrudState.Active(_, _, data), ())) =>
        assertEquals(data, "created")
      case other => fail(s"Expected Right with Active, got: $other")
  }

  test("SaaSCQRSService: modifyS updates state") {
    val active = CrudState.Active(tenantA, userA, "old")
    val cmd = CommandMessage(
      "c1",
      java.time.Instant.now(),
      "e1",
      SaaSCommand(CallerIdentity(tenantA, userA, Set.empty), "update")
    )
    val result = TestCQRSService().run(cmd, active)
    result.result match
      case Right((CrudState.Active(_, _, data), ())) =>
        assertEquals(data, "updated")
      case other => fail(s"Expected Right with updated, got: $other")
  }

  test("SaaSCQRSService: decideS transitions state") {
    val active = CrudState.Active(tenantA, userA, "old")
    val cmd = CommandMessage(
      "c1",
      java.time.Instant.now(),
      "e1",
      SaaSCommand(CallerIdentity(tenantA, userA, Set.empty), "decideS")
    )
    val result = TestCQRSService().run(cmd, active)
    result.result match
      case Right((CrudState.Active(_, _, data), ())) =>
        assertEquals(data, "decided")
      case other => fail(s"Expected Right with decided, got: $other")
  }

  test("SaaSCQRSService: decideS rejects on invalid state") {
    val cmd = CommandMessage(
      "c1",
      java.time.Instant.now(),
      "e1",
      SaaSCommand(CallerIdentity(tenantA, userA, Set.empty), "decideS")
    )
    val result = TestCQRSService().run(cmd, CrudState.NonExistent)
    assert(result.result.isLeft, s"Expected Left, got: ${result.result}")
  }

  test("SaaSCQRSService: eval runs effect") {
    val active = CrudState.Active(tenantA, userA, "data")
    val cmd = CommandMessage(
      "c1",
      java.time.Instant.now(),
      "e1",
      SaaSCommand(CallerIdentity(tenantA, userA, Set.empty), "eval")
    )
    val result = TestCQRSService().run(cmd, active)
    assert(result.result.isRight)
  }

  // ========================================================================
  // Event-sourced service tests
  // ========================================================================

  // Simple event-sourced model for testing
  enum TestEvent:
    case Created(value: String)
    case Updated(value: String)

  object TestESModel extends DomainModel[CrudState[String], TestEvent, String]:
    def initial: CrudState[String] = CrudState.NonExistent
    def transition: TestEvent => CrudState[String] => ValidatedNec[
      String,
      CrudState[String]
    ] =
      case TestEvent.Created(v) =>
        _ =>
          Validated.validNec[String, CrudState[String]](
            CrudState.Active(TenantId("t"), UserId("u"), v)
          )
      case TestEvent.Updated(v) => {
        case CrudState.Active(tid, oid, _) =>
          Validated.validNec[String, CrudState[String]](
            CrudState.Active(tid, oid, v)
          )
        case _ => Validated.invalidNec[String, CrudState[String]]("not active")
      }

  import TestESModel.given

  object TestESService
      extends SaaSEventSourcedService[
        String,
        String,
        TestEvent,
        String,
        String
      ](
        rolesFor = _ => Set.empty,
        mkRejection = identity
      ):
    def apply(): App[Id, Unit] = SaaS.guardedRouter {
      case "create" =>
        (
          CrudAction.Create,
          SaaS.decide(Decision.accept(TestEvent.Created("new")))
        )
      case _ =>
        (CrudAction.Read, SaaS.unit)
    }

  test("SaaSEventSourcedService: domain field is accessible") {
    val _ = TestESService.domain // compiles = accessible
  }

  test("SaaSEventSourcedService: guardedRouter creates events") {
    val ctx = RequestContext(
      CommandMessage(
        "c1",
        java.time.Instant.now(),
        "e1",
        SaaSCommand(CallerIdentity(tenantA, userA, Set.empty), "create")
      ),
      CrudState.NonExistent: CrudState[String]
    )
    val result = TestESService().run(ctx)
    result.result match
      case Decision.Accepted(evs, _) =>
        assertEquals(evs.toList, List(TestEvent.Created("new")))
      case other => fail(s"Expected Accepted, got: $other")
  }

  // ========================================================================
  // CrudState pattern tests
  // ========================================================================

  test("CrudState.NonExistent is the initial state") {
    assertEquals(
      TestCQRSModel.initial,
      CrudState.NonExistent: CrudState[String]
    )
  }

  test("CrudState.Active carries tenantId, ownerId, data") {
    val state: CrudState[String] = CrudState.Active(tenantA, userA, "data")
    state match
      case CrudState.Active(tid, oid, d) =>
        assertEquals(tid, tenantA)
        assertEquals(oid, userA)
        assertEquals(d, "data")
      case other => fail(s"Expected Active, got: $other")
  }

  test("CrudState.Deleted carries tenantId, ownerId") {
    val state: CrudState[String] = CrudState.Deleted(tenantA, userA)
    state match
      case CrudState.Deleted(tid, oid) =>
        assertEquals(tid, tenantA)
        assertEquals(oid, userA)
      case other => fail(s"Expected Deleted, got: $other")
  }

  // ========================================================================
  // types.scala coverage
  // ========================================================================

  test("TenantId: round-trips through apply and value") {
    val tid = TenantId("abc")
    assertEquals(tid.value, "abc")
  }

  test("UserId: round-trips through apply and value") {
    val uid = UserId("xyz")
    assertEquals(uid.value, "xyz")
  }

  test("SaaSCommand: wraps caller and payload") {
    val caller = CallerIdentity(tenantA, userA, Set("r"))
    val cmd = SaaSCommand(caller, "my-payload")
    assertEquals(cmd.caller, caller)
    assertEquals(cmd.payload, "my-payload")
  }

  test("SaaSCommand: covariant in C") {
    val cmd: SaaSCommand[Any] =
      SaaSCommand(CallerIdentity(tenantA, userA, Set.empty), "hello")
    assertEquals(cmd.payload, "hello")
  }

  test("CrudAction: all four cases exist") {
    val actions = List(
      CrudAction.Create,
      CrudAction.Read,
      CrudAction.Update,
      CrudAction.Delete
    )
    assertEquals(actions.length, 4)
    assertEquals(actions.distinct.length, 4)
  }

  // ========================================================================
  // package.scala re-export coverage
  // ========================================================================

  test("package re-exports: CommandMessage is accessible") {
    val cmd: edomata.saas.CommandMessage[String] =
      edomata.core.CommandMessage("id", java.time.Instant.now(), "addr", "pay")
    assertEquals(cmd.id, "id")
  }

  test("package re-exports: Decision is accessible") {
    val d: edomata.saas.Decision[String, Int, Unit] = Decision.accept(1)
    assert(d.isAccepted)
  }

  test("package re-exports: MessageMetadata is accessible") {
    val m: edomata.saas.MessageMetadata = MessageMetadata(None, None)
    assertEquals(m.correlation, None)
  }
}
