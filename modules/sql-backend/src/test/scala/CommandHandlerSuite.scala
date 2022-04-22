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

import cats.data.Chain
import cats.data.NonEmptyChain
import cats.effect.IO
import cats.implicits.*
import edomata.core.*
import edomata.syntax.all.*
import munit.CatsEffectSuite

import java.time.Instant
import java.time.OffsetDateTime
import java.util.UUID

import CommandHandlerSuite.*
import SUT.given_ModelTC_State_Event_Rejection

class CommandHandlerSuite extends CatsEffectSuite {

  test("Ignores redundant command") {
    for {
      flag <- IO.ref(false)
      app: APP = Edomaton.eval(flag.set(true))
      r <- repo(CommandState.Redundant)
      s = CommandHandler(r, app)
      _ <- s.apply(cmd).assertEquals(Right(()))
      _ <- r.listActions.assertEquals(Nil)
      _ <- flag.get.assertEquals(false)
    } yield ()
  }

  test("Appends accepted results") {
    val app: APP = SUT.dsl.decide(Decision.accept(1, 2, 3)).publish(4, 5, 6)
    val ctx = cmd.buildContext("")
    val version = 100

    for {
      r <- repo(AggregateState.Valid("", version))
      s = CommandHandler(r, app)
      _ <- s.apply(cmd).assertEquals(Right(()))
      _ <- r.listActions.assertEquals(
        List(
          FakeRepository.Actions
            .Appended(
              ctx,
              version,
              "123",
              NonEmptyChain(1, 2, 3),
              Chain(4, 5, 6)
            )
        )
      )
    } yield ()
  }

  test("Notifies indecisive results") {
    val app: APP = SUT.dsl.decide(Decision.unit).publish(4, 5, 6)
    val ctx = cmd.buildContext("")
    val version = 100

    for {
      r <- repo(AggregateState.Valid("", version))
      s = CommandHandler(r, app)
      _ <- s.apply(cmd).assertEquals(Right(()))
      _ <- r.listActions.assertEquals(
        List(
          FakeRepository.Actions.Notified(ctx, NonEmptyChain(4, 5, 6))
        )
      )
    } yield ()
  }

  test("Notifies rejection with notification") {
    val app: APP = SUT.dsl.reject("oops!").publish(4, 5, 6)
    val ctx = cmd.buildContext("")
    val version = 100

    for {
      r <- repo(AggregateState.Valid("", version))
      s = CommandHandler(r, app)
      _ <- s.apply(cmd).assertEquals("oops!".leftNec)
      _ <- r.listActions.assertEquals(
        List(
          FakeRepository.Actions.Notified(ctx, NonEmptyChain(4, 5, 6))
        )
      )
    } yield ()
  }

  test("Rejections with no notifications have no effect") {
    val app: APP = SUT.dsl.reject("oops!")
    val version = 100

    for {
      r <- repo(AggregateState.Valid("", version))
      s = CommandHandler(r, app)
      _ <- s.apply(cmd).assertEquals("oops!".leftNec)
      _ <- r.listActions.assertEquals(Nil)
    } yield ()
  }

  test("Indecisives with no notifications have no effect") {
    val app: APP = SUT.dsl.unit
    val version = 100

    for {
      r <- repo(AggregateState.Valid("", version))
      s = CommandHandler(r, app)
      _ <- s.apply(cmd).assertEquals(().rightNec)
      _ <- r.listActions.assertEquals(Nil)
    } yield ()
  }

  test("Must reject results that cause state to become conflicted") {
    val app: APP = SUT.dsl.decide(Decision.accept(-1, -2))
    val version = 100

    for {
      r <- repo(AggregateState.Valid("", version))
      s = CommandHandler(r, app)
      _ <- s.apply(cmd).assertEquals("bad event".leftNec)
      _ <- r.listActions.assertEquals(Nil)
    } yield ()
  }

  test("Must reject working on conflicted state") {
    val app: APP = SUT.dsl.unit
    val meta = EventMetadata(UUID.randomUUID, OffsetDateTime.MAX, 42, 16, "sut")
    val evMsg = EventMessage(meta, -1)
    val rejection = "don't know what to do"

    for {
      r <- repo(
        AggregateState.Conflicted("", evMsg, NonEmptyChain(rejection))
      )
      s = CommandHandler(r, app)
      _ <- s.apply(cmd).assertEquals(rejection.leftNec)
      _ <- r.listActions.assertEquals(Nil)
    } yield ()
  }

  test("Must not change raised errors") {
    val exception = new Exception("Some error!")
    val app: APP = SUT.dsl.eval(IO.raiseError(exception))
    val meta = EventMetadata(UUID.randomUUID, OffsetDateTime.MAX, 42, 16, "sut")

    for {
      r <- repo(AggregateState.Valid("", 0))
      s = CommandHandler(r, app)
      _ <- s.apply(cmd).attempt.assertEquals(exception.asLeft)
      _ <- r.listActions.assertEquals(Nil)
    } yield ()
  }
}

object CommandHandlerSuite {
  type State = String
  type Event = Int
  type Notification = Int
  type Rejection = String
  type Command = Int

  type APP = Edomaton[
    IO,
    RequestContext[Command, State],
    Rejection,
    Event,
    Notification,
    Unit
  ]

  def repo(
      state: CommandState[State, Event, Rejection]
  ): IO[FakeRepository[State, Event, Rejection, Notification]] =
    FakeRepository(state)

  object SUT extends DomainModel[State, Event, Rejection] {
    def initial = ""
    def transition = i =>
      s => if i > s.length then (s + i).validNec else "bad event".invalidNec
  }

  val noop: APP = Edomaton.unit
  val cmd = CommandMessage("", Instant.MAX, "", 1)
}
