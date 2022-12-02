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
package cqrs

import cats.data.Chain
import cats.data.NonEmptyChain
import cats.effect.IO
import cats.effect.testkit.TestControl
import cats.implicits.*
import edomata.core.*
import edomata.syntax.all.*
import munit.CatsEffectSuite

import java.time.Instant
import java.time.OffsetDateTime
import java.util.UUID
import scala.concurrent.duration.*

import CommandHandlerSuite.*
import SUT.given_StateModelTC_State

class CommandHandlerSuite extends CatsEffectSuite {

  test("Ignores redundant command") {
    for {
      flag <- IO.ref(false)
      app: APP = Stomaton.eval(flag.set(true))
      r <- repo(CommandState.Redundant)
      s = CommandHandler(r).apply(app)
      _ <- s.apply(cmd).assertEquals(Right(()))
      _ <- r.saved.assertEquals(Nil)
      _ <- flag.get.assertEquals(false)
    } yield ()
  }

  test("Saves accepted results") {
    val app: APP = SUT.dsl.publish(1, 2, 3).set("123")
    val ctx = cmd.buildContext("")
    val version = 100

    for {
      r <- repo(AggregateS("", version))
      s = CommandHandler(r).apply(app)
      _ <- s.apply(cmd).assertEquals(Right(()))
      _ <- r.assert(
        FakeRepository.Saved(
          cmd,
          version,
          "123",
          Chain(1, 2, 3)
        )
      )
    } yield ()
  }

  test("Saves results even if its indecisive") {
    val app: APP = Stomaton.set("123")
    val ctx = cmd.buildContext("")
    val version = 100

    for {
      r <- repo(AggregateS("", version))
      s = CommandHandler(r).apply(app)
      _ <- s.apply(cmd).assertEquals(Right(()))
      _ <- r.assert(
        FakeRepository.Saved(
          cmd,
          version,
          "123",
          Chain.empty
        )
      )
    } yield ()
  }

  test("Does not save results when its rejected") {
    val app: APP = Stomaton.reject("a", "b", "c")
    val ctx = cmd.buildContext("")
    val version = 100

    for {
      r <- repo(AggregateS("", version))
      s = CommandHandler(r).apply(app)
      _ <- s.apply(cmd).assertEquals(Left(NonEmptyChain("a", "b", "c")))
      _ <- r.saved.assertEquals(Nil)
    } yield ()
  }

  // test("Must not change raised errors") {
  //   val exception = new Exception("Some error!")
  //   val app: APP = SUT.dsl.eval(IO.raiseError(exception))
  //   val meta = EventMetadata(UUID.randomUUID, OffsetDateTime.MAX, 42, 16, "sut")

  //   for {
  //     r <- repo(AggregateState.Valid("", 0))
  //     s = CommandHandler(r).apply(app)
  //     _ <- s.apply(cmd).attempt.assertEquals(exception.asLeft)
  //     _ <- r.listActions.assertEquals(Nil)
  //   } yield ()
  // }

  // test("Must retry on version conflict") {
  //   for {
  //     c <- IO.ref(3)
  //     app: APP = SUT.dsl.eval(
  //       c.updateAndGet(_ - 1)
  //         .map(_ == 0)
  //         .ifM(IO.unit, IO.raiseError(BackendError.VersionConflict))
  //     )
  //     r <- repo(AggregateState.Valid("", 0))
  //     s = CommandHandler.withRetry(r, 3, 1.minute).apply(app)
  //     _ <- TestControl.executeEmbed(s.apply(cmd)).assertEquals(().asRight)
  //     _ <- r.listActions.assertEquals(Nil)
  //   } yield ()
  // }

  // test("Must fail with max retry on too many version conflicts") {
  //   for {
  //     c <- IO.ref(4)
  //     app: APP = SUT.dsl.eval(
  //       c.updateAndGet(_ - 1)
  //         .map(_ == 0)
  //         .ifM(IO.unit, IO.raiseError(BackendError.VersionConflict))
  //     )
  //     r <- repo(AggregateState.Valid("", 0))
  //     s = CommandHandler.withRetry(r, 3, 1.minute).apply(app)
  //     _ <- TestControl
  //       .executeEmbed(s.apply(cmd))
  //       .attempt
  //       .assertEquals(BackendError.MaxRetryExceeded.asLeft)
  //     _ <- r.listActions.assertEquals(Nil)
  //   } yield ()
  // }
}

object CommandHandlerSuite {
  type State = String
  type Event = Int
  type Rejection = String
  type Command = Int

  type APP =
    Stomaton[IO, CommandMessage[Command], State, Rejection, Event, Unit]

  def repo(
      state: AggregateState[State]
  ): IO[FakeRepository[State, Event]] =
    FakeRepository(state)

  object SUT extends CQRSModel[State, Rejection] {
    def initial = ""
  }

  val noop: APP = Stomaton.unit
  val cmd = CommandMessage("", Instant.MAX, "", 1)

}
