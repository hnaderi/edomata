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
import edomata.core.*
import fs2.Stream

import java.time.OffsetDateTime
import java.util.UUID

trait OutboxReader[F[_], N] {
  def read: Stream[F, OutboxItem[N]]
  def markAllAsSent(items: NonEmptyChain[OutboxItem[N]]): F[Unit]
  def markAsSent(item: OutboxItem[N], others: OutboxItem[N]*): F[Unit] =
    markAllAsSent(NonEmptyChain.of(item, others: _*))
}

final case class OutboxItem[N](
    seqNr: SeqNr,
    streamId: StreamId,
    time: OffsetDateTime,
    data: N,
    metadata: MessageMetadata
)
