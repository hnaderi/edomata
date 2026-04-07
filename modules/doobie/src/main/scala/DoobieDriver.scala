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
import cats.effect.std.SecureRandom
import cats.implicits.*
import doobie.implicits.*
import doobie.util.transactor.Transactor
import edomata.backend.PGNaming
import edomata.backend.PGNamespace
import edomata.backend.eventsourcing.*
import edomata.core.*

final class DoobieDriver[F[_]: Async] private (
    naming: PGNaming,
    pool: Transactor[F],
    autoSetup: Boolean
) extends StorageDriver[F, BackendCodec] {

  def build[S, E, R, N](
      snapshot: SnapshotStore[F, S]
  )(using
      model: ModelTC[S, E, R],
      event: BackendCodec[E],
      notifs: BackendCodec[N]
  ): Resource[F, Storage[F, S, E, R, N]] = {
    val jQ = Queries.Journal(naming, event)
    val nQ = Queries.Outbox(naming, notifs)
    val cQ = Queries.Commands(naming)

    def setup =
      if autoSetup then
        (jQ.setup.run >> nQ.setup.run >> cQ.setup.run)
          .as((jQ, nQ, cQ))
          .transact(pool)
      else Async[F].pure((jQ, nQ, cQ))

    Resource.eval(
      for {
        _ <- setup
        updates <- Notifications[F]
        sr <- SecureRandom.javaSecuritySecureRandom[F]
        given SecureRandom[F] = sr
        _outbox = DoobieOutboxReader(pool, nQ)
        _journal = DoobieJournalReader(pool, jQ)
        _repo = RepositoryReader(_journal, snapshot)
        skRepo = DoobieRepository(pool, jQ, nQ, cQ, _repo, updates)
      } yield Storage(skRepo, _repo, _journal, _outbox, updates)
    )
  }

  def snapshot[S](using
      BackendCodec[S]
  ): Resource[F, SnapshotPersistence[F, S]] =
    Resource.eval(DoobieSnapshotPersistence(pool, naming, autoSetup))
}

object DoobieDriver {
  inline def apply[F[_]: Async](
      inline namespace: String,
      pool: Transactor[F]
  ): F[DoobieDriver[F]] =
    from(PGNamespace(namespace), pool)

  def from[F[_]: Async](
      namespace: PGNamespace,
      pool: Transactor[F]
  ): F[DoobieDriver[F]] = from(PGNaming.schema(namespace), pool)

  def from[F[_]: Async](
      naming: PGNaming,
      pool: Transactor[F],
      skipSetup: Boolean = false
  ): F[DoobieDriver[F]] =
    val setup: F[Unit] =
      if !skipSetup && naming.needsSchemaSetup then
        Queries.setupSchema(naming.namespace).run.transact(pool).void
      else Async[F].unit
    setup.as(new DoobieDriver(naming, pool, autoSetup = !skipSetup))
}
