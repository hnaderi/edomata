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
import edomata.backend.cqrs.*
import edomata.core.*
import edomata.syntax.all.*

import java.time.Instant

abstract class CqrsSuite[S, R, N](
    backend: Resource[IO, Backend[IO, Int, String, Int]],
    name: String
) extends StorageSuite(backend, name) {
  import TestCQRSModel.given_StateModelTC_State

  private val dsl = TestCQRSDomain.dsl
  private val rndString = UUIDGen.randomString[IO]

  check("inserts state") { b =>
    val srv = b.compile(dsl.set(2))
    for {
      aggId <- randomString
      cmdId <- randomString
      _ <- srv(CommandMessage(cmdId, Instant.EPOCH, aggId, 0))
        .assertEquals(Right(()))
      _ <- b.repository
        .get(aggId)
        .assertEquals(AggregateS(version = 0, state = 2))
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
        .assertEquals(AggregateS(version = 1, state = 5))
    } yield ()
  }

  check("publishes notifications") { b =>
    val srv = b.compile(dsl.publish(1, 2, 3))
    for {
      aggId <- randomString
      cmdId <- randomString
      _ <- srv(CommandMessage(cmdId, Instant.EPOCH, aggId, 0))
        .assertEquals(Right(()))
      _ <- b.repository.get(aggId).assertEquals(AggregateS(0, 0))
      _ <- b.outbox.read
        .filter(_.streamId == aggId)
        .map(_.data)
        .compile
        .toList
        .assertEquals(List(1, 2, 3))
    } yield ()
  }

}

object TestCQRSModel extends CQRSModel[Int, String] {
  def initial: Int = 0
}
val TestCQRSDomain = TestCQRSModel.domain[Int, Int]
