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

package edomata.doobie

import _root_.doobie.Transactor
import _root_.doobie.implicits.*
import cats.effect.Concurrent
import edomata.backend.*
import fs2.Stream

private final class DoobieJournalReader[F[_]: Concurrent, E](
    trx: Transactor[F],
    qs: Queries.Journal[E]
) extends JournalReader[F, E] {
  def readStream(streamId: StreamId): Stream[F, EventMessage[E]] =
    qs.readStream(streamId).stream.transact(trx)
  def readStreamAfter(
      streamId: StreamId,
      version: EventVersion
  ): Stream[F, EventMessage[E]] =
    qs.readStreamAfter(streamId, version).stream.transact(trx)

  def readStreamBefore(
      streamId: StreamId,
      version: EventVersion
  ): Stream[F, EventMessage[E]] =
    qs.readStreamBefore(streamId, version).stream.transact(trx)

  def readAll: Stream[F, EventMessage[E]] = qs.readAll.stream.transact(trx)
  def readAllAfter(seqNr: SeqNr): Stream[F, EventMessage[E]] =
    qs.readAllAfter(seqNr).stream.transact(trx)
  def readAllBefore(seqNr: SeqNr): Stream[F, EventMessage[E]] =
    qs.readAllBefore(seqNr).stream.transact(trx)
}
