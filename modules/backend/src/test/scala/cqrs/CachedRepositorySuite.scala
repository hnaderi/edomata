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

import cats.data.*
import cats.effect.IO
import cats.implicits.*
import edomata.core.*
import munit.CatsEffectSuite

import java.time.Instant

import CachedRepositorySuite.*
import FakeRepository.Interaction

class CachedRepositorySuite extends CatsEffectSuite {
  test("save must use underlying and also update caches") {
    for {
      und <- FakeRepository(AggregateS("", 1))
      cache <- Cache.lru[IO, StreamId, AggregateS[String]](1)
      cmds <- FakeCommandStore()
      repo = CachedRepository.from(und, cmds, cache)

      _ <- repo.save(someCmd, 2, "state", Chain(4, 5, 6))

      _ <- und.assert(Interaction.Saved(someCmd, 2, "state", Chain(4, 5, 6)))
      _ <- cache.get("sut").assertEquals(AggregateS("state", 3).some)
      _ <- cmds.all.assertEquals(Set("cmdId"))
    } yield ()
  }

  test("save must not update when underlying fails") {
    for {
      cache <- Cache.lru[IO, StreamId, AggregateS[String]](1)
      cmds <- FakeCommandStore()
      repo = CachedRepository.from(FailingRepository(), cmds, cache)

      _ <- repo.save(someCmd, 2, "state", Chain(4, 5, 6)).attempt

      _ <- cache.get("sut").assertEquals(None)
      _ <- cmds.all.assertEquals(Set())
    } yield ()
  }

  test("cache updating must be convergent") {
    for {
      und <- FakeRepository(AggregateS("", 1))
      cache <- Cache.lru[IO, StreamId, AggregateS[String]](1)
      cmds <- FakeCommandStore()
      repo = CachedRepository.from(und, cmds, cache)

      _ <- repo.save(someCmd.copy(id = "new"), 3, "state new", Chain.nil)
      _ <- repo.save(someCmd.copy(id = "old"), 2, "state old", Chain(4, 5, 6))

      _ <- cache.get("sut").assertEquals(AggregateS("state new", 4).some)
      _ <- cmds.all.assertEquals(Set("new", "old"))
    } yield ()
  }

  test("get must use underlying") {
    for {
      und <- FakeRepository(AggregateS("", 1))
      repo <- CachedRepository.build(und)

      // Even if it is in cache!
      _ <- repo.save(someCmd, 2, "new state", Chain.nil)

      _ <- repo.get("sut").assertEquals(AggregateS("", 1))
    } yield ()
  }

  test("notify must use underlying") {
    for {
      und <- FakeRepository(AggregateS("", 1))
      repo <- CachedRepository.build(und)
      _ <- repo.notify(someCmd, NonEmptyChain(1, 2, 3))

      _ <- und.assert(Interaction.Notified(someCmd, NonEmptyChain(1, 2, 3)))
    } yield ()
  }

  test("load must ignore underlying if cache has the required data") {
    for {
      und <- FakeRepository(AggregateS("", 1))
      repo <- CachedRepository.build(und)
      _ <- repo.save(someCmd, 2, "new state", Chain.nil)

      _ <- repo.load(someCmd).assertEquals(CommandState.Redundant)
      _ <- repo
        .load(someCmd.copy(id = "new cmdId"))
        .assertEquals(AggregateS("new state", 3))
    } yield ()
  }

  test("load must use underlying if cache does not have the required data") {
    for {
      und <- FakeRepository(AggregateS("underlying", 4))
      repo <- CachedRepository.build(und)

      _ <- repo.load(someCmd).assertEquals(AggregateS("underlying", 4))
    } yield ()
  }
}

object CachedRepositorySuite {
  val someCmd = CommandMessage("cmdId", Instant.MAX, "sut", 1)

  private case object PlannedFailure extends Exception("Planned to fail")
  private val fail = IO.raiseError(PlannedFailure)

  final class FailingRepository extends Repository[IO, String, Int] {

    override def get(id: StreamId): IO[AggregateS[String]] = fail

    override def load(cmd: CommandMessage[?]): IO[AggregateState[String]] = fail

    override def save(
        ctx: CommandMessage[?],
        version: SeqNr,
        newState: String,
        events: Chain[Int]
    ): IO[Unit] = fail

    override def notify(
        ctx: CommandMessage[?],
        notifications: NonEmptyChain[Int]
    ): IO[Unit] = fail

  }
}
