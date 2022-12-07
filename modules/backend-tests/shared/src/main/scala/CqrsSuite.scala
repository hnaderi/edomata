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

package tests

import cats.effect.IO
import cats.effect.kernel.Resource
import cats.effect.std.UUIDGen
import cats.implicits.*
import edomata.backend.BackendError
import edomata.backend.cqrs.*
import edomata.core.*
import edomata.syntax.all.*

import java.time.Instant

abstract class CqrsSuite[S, R, N](
    backend: Resource[IO, Backend[IO, Int, String, Int]],
    name: String
) extends StorageSuite(backend, name) {
  import TestCQRSModel.given_StateModelTC_State

  type SUT = Backend[IO, Int, String, Int]
  private val dsl = TestCQRSDomain.dsl
  private val someCmd = (randomString, randomString).mapN(
    CommandMessage(_, Instant.EPOCH, _, 0)
  )

  check("inserts state") { b =>
    val srv = b.compile(dsl.set(2))
    for {
      aggId <- randomString
      cmdId <- randomString
      _ <- srv(CommandMessage(cmdId, Instant.EPOCH, aggId, 0))
        .assertEquals(Right(()))
      _ <- b.repository
        .get(aggId)
        .assertEquals(AggregateState(version = 1, state = 2))

      _ <- assertNotifiedState(b)
    } yield ()
  }

  check("updates existing state") { b =>
    val srv = b.compile(dsl.router(i => dsl.set(i)))
    for {
      aggId <- randomString

      cmdId <- randomString
      _ <- srv(CommandMessage(cmdId, Instant.EPOCH, aggId, 2))

      cmdId2 <- randomString
      _ <- srv(CommandMessage(cmdId2, Instant.EPOCH, aggId, 5))
        .assertEquals(Right(()))

      _ <- b.repository
        .get(aggId)
        .assertEquals(AggregateState(version = 2, state = 5))

      _ <- assertNotifiedState(b)
    } yield ()
  }

  check("publishes notifications") { b =>
    val srv = b.compile(dsl.publish(1, 2, 3))
    for {
      aggId <- randomString
      cmdId <- randomString
      _ <- srv(CommandMessage(cmdId, Instant.EPOCH, aggId, 0))
        .assertEquals(Right(()))
      _ <- b.repository
        .get(aggId)
        .assertEquals(AggregateState(version = 1, state = 0))
      _ <- b.outbox.read
        .filter(_.streamId == aggId)
        .map(_.data)
        .compile
        .toList
        .assertEquals(List(1, 2, 3))

      _ <- assertNotifiedOutbox(b)
    } yield ()
  }

  check("save must be idempotent") { s =>
    for {
      cmd <- someCmd
      append = s
        .compile(dsl.modify[IO](_ + 10).publish(4, 5, 6).void)
        .apply(cmd)
        .attempt

      _ <- List
        .fill(20)(append)
        .parSequence
        .map(ats =>
          ats.foreach {
            case Right(_)                           => ()
            case Left(BackendError.VersionConflict) => ()
            case Left(other) => fail("Invalid implementation", other)
          }
        )

      _ <- s.repository
        .get(cmd.address)
        .assertEquals(
          AggregateState(
            state = 10,
            version = 1
          )
        )
      _ <- assertNotifiedState(s)
      _ <- assertNotifiedOutbox(s)
    } yield ()
  }

  check("save must be correct") { s =>
    for {
      aggId <- randomString
      append = someCmd
        .map(_.copy(address = aggId))
        .flatMap(s.compile(dsl.modify[IO](_ + 10).publish(4, 5, 6).void))
        .attempt

      _ <- List
        .fill(5)(append)
        .parSequence
        .map(ats =>
          ats.foreach {
            case Right(_)                           => ()
            case Left(BackendError.VersionConflict) => fail("bad luck!")
            case Left(other) => fail("Invalid implementation", other)
          }
        )

      _ <- s.repository
        .get(aggId)
        .assertEquals(
          AggregateState(
            state = 50,
            version = 5
          )
        )
      _ <- assertNotifiedState(s)
      _ <- assertNotifiedOutbox(s)
    } yield ()
  }

  private def assertNotifiedState(s: SUT) =
    s.updates.state.head.compile.lastOrError.assertEquals(())
  private def assertNotifiedOutbox(s: SUT) =
    s.updates.outbox.head.compile.lastOrError.assertEquals(())
}

object TestCQRSModel extends CQRSModel[Int, String] {
  def initial: Int = 0
}
val TestCQRSDomain = TestCQRSModel.domain[Int, Int]
