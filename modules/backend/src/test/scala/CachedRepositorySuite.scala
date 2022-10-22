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
import munit.CatsEffectSuite

import java.time.Instant

import CachedRepositorySuite.*

class CachedRepositorySuite extends CatsEffectSuite {
  test("Must fallback to underlying repository when can't optimize") {
    for {
      repo <- FakeRepository(persistedState)
      cr = CachedRepository(
        repo,
        BlackHoleCommandStore,
        new BlackHoleSnapshotStore
      )
      _ <- cr.load(someCmd).assertEquals(persistedState)
      _ <- repo.listLoaded.assertEquals(List(someCmd))
    } yield ()
  }
  test("Must short circuit when knows it's a redundant command") {
    for {
      repo <- FakeRepository(persistedState)
      cr = CachedRepository(
        repo,
        YesManCommandStore,
        new BlackHoleSnapshotStore
      )
      _ <- cr.load(someCmd).assertEquals(CommandState.Redundant)
      _ <- repo.listLoaded.assertEquals(Nil)
    } yield ()
  }
  test("Must use its own snapshot if not empty") {
    for {
      repo <- FakeRepository(persistedState)
      cr = CachedRepository(
        repo,
        BlackHoleCommandStore,
        new ConstantSnapshotStore(10, 3)
      )
      _ <- cr.load(someCmd).assertEquals(inMemState)
      _ <- repo.listLoaded.assertEquals(Nil)
    } yield ()
  }
  test("Must not use cold snapshot") {
    for {
      repo <- FakeRepository(persistedState)
      cr = CachedRepository(
        repo,
        BlackHoleCommandStore,
        new LaggedSnapshotStore(10, 3, 1)
      )
      _ <- cr.load(someCmd).assertEquals(inMemState)
      _ <- repo.listLoaded.assertEquals(Nil)
    } yield ()
  }

  test("Must update its commands and states on successful append") {
    val events = NonEmptyChain(1, 2, 3)
    val notifs = Chain(4, 5, 6)
    val newState = 11
    val version = 1

    for {
      s <- FakeSnapShotStore[Int]()
      c <- FakeCommandStore()
      repo <- FakeRepository(persistedState)
      cr = CachedRepository(repo, c, s)

      _ <- cr.append(someCtx, version, newState, events, notifs)

      _ <- s.all.assertEquals(
        Map(
          someCmd.address -> AggregateState.Valid(
            newState,
            version + events.size
          )
        )
      )
      _ <- c.all.assertEquals(Set(someCmd.id))
      _ <- repo.listActions.assertEquals(
        List(
          FakeRepository.Actions.Appended(
            someCtx,
            version = version,
            newState = newState,
            events,
            notifs
          )
        )
      )
    } yield ()
  }
  test("Must not update its commands and states on failed append") {
    for {
      s <- FakeSnapShotStore[Int]()
      c <- FakeCommandStore()
      repo = new FailingRepository[Int, Int, Nothing, Int]
      cr = CachedRepository(repo, c, s)

      _ <- cr
        .append(someCtx, 1, 11, NonEmptyChain(1, 2, 3), Chain(4, 5, 6))
        .attempt
        .assertEquals(PlanedFailure.asLeft)

      _ <- s.all.assertEquals(Map.empty)
      _ <- c.all.assertEquals(Set.empty)
    } yield ()
  }

  test("Must notify using underlying") {
    val notifs = NonEmptyChain(4, 5, 6)

    for {
      s <- FakeSnapShotStore[Int]()
      c <- FakeCommandStore()
      repo <- FakeRepository(persistedState)
      cr = CachedRepository(repo, c, s)

      _ <- cr.notify(someCtx, notifs)

      _ <- s.all.assertEquals(Map.empty)
      _ <- c.all.assertEquals(Set.empty)
      _ <- repo.listActions.assertEquals(
        List(
          FakeRepository.Actions.Notified(
            someCtx,
            notifs
          )
        )
      )
    } yield ()
  }
}

object CachedRepositorySuite {
  val someCmd = CommandMessage("", Instant.MAX, "sut", 1)
  val someCtx = someCmd.buildContext(1)
  val inMemState: AggregateState[Int, Int, Nothing] =
    AggregateState.Valid(10, 3)
  val persistedState: AggregateState[Int, Int, Nothing] =
    AggregateState.Valid(1, 0)
}
