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

package edomata.skunk

import _root_.skunk.*
import cats.effect.Concurrent
import cats.effect.kernel.Resource
import edomata.backend.*
import fs2.Stream

private final class SkunkJournalReader[F[_]: Concurrent, E](
    pool: Resource[F, Session[F]],
    q: Queries.Journal[E]
) extends JournalReader[F, E] {

  private def run[A, B](q: Query[A, B])(a: A) =
    Stream.resource(pool).evalMap(_.prepare(q)).flatMap(_.stream(a, 100))

  def readStream(streamId: StreamId): Stream[F, EventMessage[E]] =
    run(q.readStream)(streamId)

  def readStreamAfter(
      streamId: StreamId,
      version: EventVersion
  ): Stream[F, EventMessage[E]] = run(q.readStreamAfter)((streamId, version))

  def readStreamBefore(
      streamId: StreamId,
      version: EventVersion
  ): Stream[F, EventMessage[E]] = run(q.readStreamBefore)((streamId, version))

  def readAll: Stream[F, EventMessage[E]] = run(q.readAll)(Void)

  def readAllAfter(seqNr: SeqNr): Stream[F, EventMessage[E]] =
    run(q.readAllAfter)(seqNr)

  def readAllBefore(seqNr: SeqNr): Stream[F, EventMessage[E]] =
    run(q.readAllBefore)(seqNr)
}
