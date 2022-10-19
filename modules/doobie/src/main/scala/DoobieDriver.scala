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

import cats.effect.kernel.Async
import cats.effect.kernel.Resource
import cats.implicits.*
import doobie.implicits.*
import doobie.util.transactor.Transactor
import edomata.backend.*
import edomata.core.*

final class DoobieDriver[F[_]: Async, S, E, R, N](
    namespace: PGNamespace,
    pool: Transactor[F]
) extends StorageDriver[F, BackendCodec, S, E, R, N] {

  def build(
      snapshot: SnapshotStore[F, S]
  )(using
      model: ModelTC[S, E, R],
      event: BackendCodec[E],
      notifs: BackendCodec[N]
  ): Resource[F, Storage[F, S, E, R, N]] = {
    val jQ = Queries.Journal(namespace, event)
    val nQ = Queries.Outbox(namespace, notifs)
    val cQ = Queries.Commands(namespace)

    def setup =
      (jQ.setup.run >> nQ.setup.run >> cQ.setup.run)
        .as((jQ, nQ, cQ))
        .transact(pool)

    for {
      updates <- Resource.eval(Notifications[F])
      _outbox = DoobieOutboxReader(pool, nQ)
      _journal = DoobieJournalReader(pool, jQ)
      _repo = RepositoryReader(_journal, snapshot)
      skRepo = DoobieRepository(pool, jQ, nQ, cQ, _repo, updates)
    } yield Storage(skRepo, _repo, _journal, _outbox, updates)
  }

  def snapshot(using BackendCodec[S]): Resource[F, SnapshotPersistence[F, S]] =
    Resource.eval(DoobieSnapshotPersistence(pool, namespace))
}

object DoobieDriver {
  inline def apply[F[_]: Async, S, E, R, N](
      inline namespace: String,
      pool: Transactor[F]
  ): F[DoobieDriver[F, S, E, R, N]] =
    from(PGNamespace(namespace), pool)

  def from[F[_]: Async, S, E, R, N](
      namespace: PGNamespace,
      pool: Transactor[F]
  ): F[DoobieDriver[F, S, E, R, N]] =
    Queries
      .setupSchema(namespace)
      .run
      .transact(pool)
      .as(new DoobieDriver(namespace, pool))
}
