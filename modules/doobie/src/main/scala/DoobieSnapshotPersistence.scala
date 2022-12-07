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
import cats.implicits.*
import edomata.backend.PGNamespace
import edomata.backend.StreamId
import edomata.backend.eventsourcing.*
import edomata.core.*
import fs2.Chunk
import fs2.Stream

import java.time.OffsetDateTime
import java.time.ZoneOffset

private final class DoobieSnapshotPersistence[F[_]: Concurrent, S](
    trx: Transactor[F],
    qs: Queries.Snapshot[S]
) extends SnapshotPersistence[F, S] {
  def get(id: StreamId): F[Option[AggregateState.Valid[S]]] =
    qs.get(id).option.transact(trx)
  def put(items: Chunk[SnapshotItem[S]]): F[Unit] =
    qs.put(items.toList).transact(trx).void
}

private object DoobieSnapshotPersistence {
  def apply[F[_]: Concurrent, S](
      pool: Transactor[F],
      namespace: PGNamespace
  )(using codec: BackendCodec[S]): F[DoobieSnapshotPersistence[F, S]] =
    val q = Queries.Snapshot[S](namespace, codec)
    q.setup.run.transact(pool).as(new DoobieSnapshotPersistence(pool, q))
}
