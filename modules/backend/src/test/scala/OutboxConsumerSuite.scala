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

import cats.data.NonEmptyChain
import cats.effect.IO
import cats.implicits.*
import edomata.core.MessageMetadata
import fs2.Stream
import fs2.Stream.*
import munit.CatsEffectSuite
import munit.FunSuite

import java.time.OffsetDateTime

class OutboxConsumerSuite extends CatsEffectSuite {
  private def backend(
      fo: FakeOutboxReader[Int]
  ): Backend[IO, Int, Int, Int, Int] = new {
    def compile: CommandHandler[IO, Int, Int, Int, Int] = ???
    def outbox: OutboxReader[IO, Int] = fo
    def journal: JournalReader[IO, Int] = ???
    def repository: RepositoryReader[IO, Int, Int, Int] = ???
    def updates: NotificationsConsumer[IO] = new {
      def journal: Stream[IO, Unit] = empty
      def outbox: Stream[IO, Unit] = empty
    }
  }

  private def itemsFor(s: Stream[IO, Int]): Stream[IO, OutboxItem[Int]] =
    s.zipWithIndex.map((i, id) =>
      OutboxItem(
        seqNr = id,
        streamId = "sut",
        time = OffsetDateTime.MIN.plusDays(id),
        data = i,
        metadata = MessageMetadata(id.toString)
      )
    )

  test("Empty") {
    for {
      fo <- FakeOutboxReader[Int](empty)
      b = backend(fo)
      _ <- OutboxConsumer(b)(_ => IO(fail("How in the world?"))).compile.drain
    } yield ()
  }

  test("Must run action on all consumed items") {
    for {
      fo <- FakeOutboxReader(itemsFor(range(10, 20)))
      b = backend(fo)
      counter <- IO.ref(0)
      _ <- OutboxConsumer(b) { item =>
        counter.getAndUpdate(_ + 1).assertEquals(item.seqNr) >>
          IO {
            assertEquals(item.data.toLong, item.seqNr + 10)
            assertEquals(item.time, OffsetDateTime.MIN.plusDays(item.seqNr))
            assertEquals(item.streamId, "sut")
          }
      }.compile.drain
      _ <- counter.get.assertEquals(10)
    } yield ()
  }

  test("Must mark each chunk as read after successful run") {
    val data = itemsFor(
      Stream(
        emit(1),
        range(2, 5),
        range(6, 20)
      ).flatMap(_.chunkAll.unchunks)
    )
    for {
      fo <- FakeOutboxReader(data)
      b = backend(fo)
      _ <- OutboxConsumer(b)(_ => IO.unit).compile.drain
      m <- fo.listActions
    } yield m match {
      case c :: b :: a :: Nil =>
        assertMarked(a, 1)
        assertMarked(b, 2, 5)
        assertMarked(c, 6, 20)
      case _ => fail("Invalid interaction!")
    }
  }

  private def assertMarked(
      l: FakeOutboxReader.Marked[Int],
      from: Int,
      to: Int
  ) =
    assertEquals(l.items.map(_.data).toList, List.range(from, to))
  private def assertMarked(l: FakeOutboxReader.Marked[Int], single: Int) =
    assertEquals(l.items.map(_.data).toList, List(single))
}
