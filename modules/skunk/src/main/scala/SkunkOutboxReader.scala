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
import cats.data.NonEmptyChain
import cats.effect.Concurrent
import cats.effect.kernel.Clock
import cats.effect.kernel.Resource
import cats.implicits.*
import edomata.backend.*
import fs2.Stream

private final class SkunkOutboxReader[F[_]: Concurrent: Clock, N](
    pool: Resource[F, Session[F]],
    q: Queries.Outbox[N]
) extends OutboxReader[F, N] {
  def read: Stream[F, OutboxItem[N]] = Stream
    .resource(pool)
    .evalMap(_.prepare(q.read))
    .flatMap(_.stream(Void, 100))

  def markAllAsSent(items: NonEmptyChain[OutboxItem[N]]): F[Unit] = for {
    now <- currentTime[F]
    is = items.toList.map(_.seqNr)
    _ <- pool.use(
      _.prepare(q.markAsPublished(is)).flatMap(_.execute((now, is)))
    )
  } yield ()
}
