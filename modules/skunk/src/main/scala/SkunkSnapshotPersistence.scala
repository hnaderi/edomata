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

import _root_.skunk.Session
import cats.effect.Concurrent
import cats.effect.kernel.Resource
import cats.implicits.*
import edomata.backend.PGNamespace
import edomata.backend.StreamId
import edomata.backend.eventsourcing.*
import fs2.Chunk

private final class SkunkSnapshotPersistence[F[_]: Concurrent, S](
    pool: Resource[F, Session[F]],
    q: Queries.Snapshot[S]
) extends SnapshotPersistence[F, S] {
  def get(id: StreamId): F[Option[AggregateState.Valid[S]]] =
    pool.use(_.prepare(q.get).flatMap(_.option(id)))
  def put(items: Chunk[SnapshotItem[S]]): F[Unit] = {
    val l = SnapshotStore.dedup(items).toList
    pool.use(_.prepare(q.put(l)).flatMap(_.execute(l))).void
  }
}

private object SkunkSnapshotPersistence {
  def apply[F[_]: Concurrent, S](
      pool: Resource[F, Session[F]],
      namespace: PGNamespace
  )(using codec: BackendCodec[S]): F[SkunkSnapshotPersistence[F, S]] =
    val q = Queries.Snapshot[S](namespace, codec)
    pool.use(_.execute(q.setup)).as(new SkunkSnapshotPersistence(pool, q))
}
