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

import cats.data.NonEmptyChain
import cats.effect.IO
import cats.effect.kernel.Resource
import cats.implicits.*
import edomata.backend.BackendError
import edomata.backend.eventsourcing.Backend
import edomata.core.CommandMessage
import edomata.core.Edomaton
import edomata.core.ResponseD
import munit.Location
import tests.TestDomain.given_ModelTC_State_Event_Rejection

import java.time.Instant

abstract class PersistenceSuite(
    storage: Resource[IO, Backend[IO, Int, Int, String, Int]],
    name: String
) extends StorageSuite(storage, name) {

  private val someCmd = (randomString, randomString).mapN(
    CommandMessage(_, Instant.EPOCH, _, "command")
  )

  type SUT = Backend[IO, Int, Int, String, Int]

  private def assertJournal(
      s: SUT,
      address: String
  )(evs: Int*)(using Location) = s.journal
    .readStream(address)
    .map(_.payload)
    .compile
    .toList
    .assertEquals(evs.toList)

  private def assertOutbox(
      s: SUT,
      address: String
  )(evs: Int*)(using Location) = s.outbox.read
    .filter(_.streamId == address)
    .map(_.data)
    .compile
    .toList
    .assertEquals(evs.toList)

  private def assertNotifiedJournal(s: SUT)(using Location) =
    s.updates.journal.head.compile.lastOrError.assertEquals(())
  private def assertNotifiedOutbox(s: SUT)(using Location) =
    s.updates.outbox.head.compile.lastOrError.assertEquals(())

  check("Must append correctly") { s =>
    for {
      cmd <- someCmd
      _ <- s
        .compile(Edomaton.lift(ResponseD.accept(1, 2, 3).publish(4, 5, 6)))
        .apply(cmd)

      _ <- assertJournal(s, cmd.address)(1, 2, 3)
      _ <- assertOutbox(s, cmd.address)(4, 5, 6)

      _ <- assertNotifiedJournal(s)
      _ <- assertNotifiedOutbox(s)
    } yield ()
  }

  check("Appending must be idempotent") { s =>
    for {
      cmd <- someCmd
      append = s
        .compile(Edomaton.lift(ResponseD.accept(1, 2, 3).publish(4, 5, 6)))
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

      _ <- assertJournal(s, cmd.address)(1, 2, 3)
      _ <- assertOutbox(s, cmd.address)(4, 5, 6)

      _ <- assertNotifiedJournal(s)
      _ <- assertNotifiedOutbox(s)
    } yield ()
  }

  check("Must notify correctly") { s =>
    for {
      cmd <- someCmd
      _ <- s
        .compile(Edomaton.lift(ResponseD.publish(4, 5, 6)))
        .apply(cmd)

      _ <- s.journal
        .readStream(cmd.address)
        .compile
        .toList
        .assertEquals(Nil)

      _ <- s.outbox.read
        .filter(_.streamId == cmd.address)
        .map(_.data)
        .compile
        .toList
        .assertEquals(List(4, 5, 6))

      _ <- s.updates.outbox.head.compile.lastOrError.assertEquals(())
    } yield ()
  }

  check("Must consume outbox correctly") { s =>
    for {
      cmd <- someCmd
      _ <- s
        .compile(Edomaton.lift(ResponseD.publish(4, 5, 6)))
        .apply(cmd)

      items <- s.outbox.read.compile.toList
      _ <- IO(assert(items.length >= 3))
      consumed = items.head
      _ <- s.outbox.markAsSent(consumed)

      items2 <- s.outbox.read.compile.toList
      _ <- IO(assert(items2.length >= 2))
      _ <- IO(assertEquals(items2, items.tail))

      consumedAll = NonEmptyChain.fromSeq(items2).get
      _ <- s.outbox.markAllAsSent(consumedAll)

      items3 <- s.outbox.read.compile.toList
      _ <- IO(assert(!items3.containsSlice(items2)))
    } yield ()
  }

  check("Must read all journal") { s =>
    for {
      cmd <- someCmd
      _ <- s
        .compile(Edomaton.lift(ResponseD.accept(1, 2, 3)))
        .apply(cmd)

      events <- s.journal.readAll.take(10).compile.toList
      evSize = events.size
      _ <- IO(assert(evSize >= 3))
      sortedEvents = events.sortBy(_.metadata.seqNr)
      _ <- IO(assertEquals(events, sortedEvents))

      pivotIdx = evSize / 2
      pivot = events(pivotIdx).metadata.seqNr

      before = events.take(pivotIdx)
      after = events.drop(pivotIdx)

      _ <- s.journal
        .readAllBefore(pivot)
        .take(before.size)
        .compile
        .toList
        .assertEquals(before)

      _ <- s.journal
        .readAllAfter(pivot - 1)
        .take(after.size)
        .compile
        .toList
        .assertEquals(after)
    } yield ()
  }

  check("Must read single stream from journal") { s =>
    for {
      cmd <- someCmd
      _ <- s
        .compile(Edomaton.lift(ResponseD.accept(1, 2, 3)))
        .apply(cmd)

      events <- s.journal.readStream(cmd.address).take(10).compile.toList
      _ <- IO(assertEquals(events.size, 3))
      sortedBySeqNr = events.sortBy(_.metadata.seqNr)
      sortedByVersion = events.sortBy(_.metadata.version)
      _ <- IO(assertEquals(events, sortedBySeqNr))
      _ <- IO(assertEquals(events, sortedByVersion))
      _ <- IO(assert(events.forall(_.metadata.stream == cmd.address)))

      pivotIdx = 1
      pivot = events(pivotIdx).metadata.version

      before = events.take(pivotIdx)
      after = events.drop(pivotIdx)

      _ <- s.journal
        .readStreamBefore(cmd.address, pivot)
        .take(before.size)
        .compile
        .toList
        .assertEquals(before)

      _ <- s.journal
        .readStreamAfter(cmd.address, pivot - 1)
        .take(after.size)
        .compile
        .toList
        .assertEquals(after)
    } yield ()
  }
}
