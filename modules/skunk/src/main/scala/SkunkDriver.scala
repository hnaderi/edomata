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
import cats.effect.kernel.Async
import cats.effect.kernel.Resource
import cats.implicits.*
import edomata.backend.*
import edomata.backend.eventsourcing.*
import edomata.core.*

final class SkunkDriver[F[_]: Async] private (
    namespace: PGNamespace,
    pool: Resource[F, Session[F]]
) extends StorageDriver[F, BackendCodec] {

  def build[S, E, R, N](
      snapshot: SnapshotStore[F, S]
  )(using
      model: ModelTC[S, E, R],
      event: BackendCodec[E],
      notifs: BackendCodec[N]
  ): Resource[F, Storage[F, S, E, R, N]] = {
    def setup = {
      val jQ = Queries.Journal(namespace, event)
      val nQ = Queries.Outbox(namespace, notifs)
      val cQ = Queries.Commands(namespace)

      pool
        .use(s =>
          s.execute(jQ.setup) >> s.execute(nQ.setup) >> s.execute(cQ.setup)
        )
        .as((jQ, nQ, cQ))
    }

    Resource
      .eval(setup)
      .flatMap((jQ, nQ, cQ) =>
        for {
          updates <- Resource.eval(Notifications[F])
          _outbox = SkunkOutboxReader(pool, nQ)
          _journal = SkunkJournalReader(pool, jQ)
          _repo = RepositoryReader(_journal, snapshot)
          skRepo = SkunkRepository(pool, jQ, nQ, cQ, _repo, updates)
        } yield Storage(skRepo, _repo, _journal, _outbox, updates)
      )
  }

  def snapshot[S](using
      BackendCodec[S]
  ): Resource[F, SnapshotPersistence[F, S]] =
    Resource.eval(SkunkSnapshotPersistence(pool, namespace))
}

object SkunkDriver {
  inline def apply[F[_]: Async](
      inline namespace: String,
      pool: Resource[F, Session[F]]
  ): F[SkunkDriver[F]] = from(PGNamespace(namespace), pool)

  def from[F[_]: Async](
      namespace: PGNamespace,
      pool: Resource[F, Session[F]]
  ): F[SkunkDriver[F]] =
    pool
      .use(_.execute(Queries.setupSchema(namespace)))
      .as(new SkunkDriver(namespace, pool))
}
