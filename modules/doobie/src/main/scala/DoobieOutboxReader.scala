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
import cats.data.NonEmptyChain
import cats.effect.Concurrent
import cats.effect.kernel.Clock
import cats.implicits.*
import edomata.backend.*
import edomata.core.*
import fs2.Stream

import java.time.OffsetDateTime
import java.time.ZoneOffset

private final class DoobieOutboxReader[F[_]: Concurrent: Clock, N](
    trx: Transactor[F],
    qs: Queries.Outbox[N]
) extends OutboxReader[F, N] {
  def read: Stream[F, OutboxItem[N]] = qs.read.stream.transact(trx)
  def markAllAsSent(items: NonEmptyChain[OutboxItem[N]]): F[Unit] =
    Clock[F].realTimeInstant.flatMap(time =>
      qs.markAsPublished(items.map(_.seqNr), time.atOffset(ZoneOffset.UTC))
        .run
        .transact(trx)
        .void
    )
}
