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

import cats.effect.IO
import cats.implicits.*
import edomata.core.DomainModel
import fs2.Stream
import munit.CatsEffectSuite

import java.time.OffsetDateTime
import java.util.UUID

class RepositoryReaderSuite extends CatsEffectSuite {
  // A domain that is basically a monoid!
  private object SUTDomain extends DomainModel[Long, Int, String] {
    def initial = 0L
    def transition = i => l => (i + l).validNec
  }

  import SUTDomain.given

  def genList(streamId: String, n: Int): List[EventMessage[Int]] = List
    .range(0, n)
    .map(i =>
      EventMessage(
        EventMetadata(
          UUID(i, i),
          OffsetDateTime.MIN.plusMinutes(i),
          10 * i,
          i,
          streamId
        ),
        i + 1
      )
    )

  private val initial: AggregateState.Valid[Long] = AggregateState.Valid(0L, 0)
  private val data = genList("sut", 10)

  test("Sanity") {
    val snapshot: SnapshotReader[IO, Long] = BlackHoleSnapshotStore()
    val journal: JournalReader[IO, Int] = JournalReaderStub(data)

    val r = RepositoryReader(journal, snapshot)

    r.get("sut").assertEquals(AggregateState.Valid(55, 10)) >>
      r.history("sut")
        .compile
        .toList
        .assertEquals(
          data.scanLeft(initial)((s, e) =>
            AggregateState.Valid(s.state + e.payload, s.version + 1)
          )
        )
  }
}

class JournalReaderStub[E](data: Stream[IO, EventMessage[E]])
    extends JournalReader[IO, E] {

  def this(l: List[EventMessage[E]]) = this(Stream.emits(l))

  def readStream(streamId: StreamId): Stream[IO, EventMessage[E]] =
    data.filter(_.metadata.stream == streamId)
  def readStreamAfter(
      streamId: StreamId,
      version: EventVersion
  ): Stream[IO, EventMessage[E]] = data.filter(e =>
    e.metadata.stream == streamId && e.metadata.version > version
  )
  def readStreamBefore(
      streamId: StreamId,
      version: EventVersion
  ): Stream[IO, EventMessage[E]] = data.filter(e =>
    e.metadata.stream == streamId && e.metadata.version < version
  )

  def readAll: Stream[IO, EventMessage[E]] = data
  def readAllAfter(seqNr: SeqNr): Stream[IO, EventMessage[E]] =
    data.filter(_.metadata.seqNr > seqNr)
  def readAllBefore(seqNr: SeqNr): Stream[IO, EventMessage[E]] =
    data.filter(_.metadata.seqNr < seqNr)
}
