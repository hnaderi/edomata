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
import edomata.backend.PGNamespace
import edomata.backend.cqrs.*
import edomata.core.*

final class DoobieDriverCQRS[F[_]: Async] private (
    namespace: PGNamespace,
    pool: Transactor[F]
) extends StorageDriver[F, BackendCodec, DoobieHandler] {

  override def build[S, N, R](handler: DoobieHandler[N])(using
      tc: StateModelTC[S],
      state: BackendCodec[S],
      notifs: BackendCodec[N]
  ): Resource[F, Storage[F, S, N, R]] = {
    val sQ = Queries.State(namespace, state)
    val nQ = Queries.Outbox(namespace, notifs)
    val cQ = Queries.Commands(namespace)

    def setup =
      (sQ.setup.run >> nQ.setup.run >> cQ.setup.run)
        .as((sQ, nQ, cQ))
        .transact(pool)

    Resource.eval {
      for {
        _ <- setup
        updates <- Notifications[F]
        outbox = DoobieOutboxReader(pool, nQ)
        repo = DoobieCQRSRepository(pool, sQ, nQ, cQ, updates, handler)
      } yield Storage(repo, outbox, updates)
    }
  }

  override def build[S, N, R](using
      StateModelTC[S],
      BackendCodec[S],
      BackendCodec[N]
  ): Resource[F, Storage[F, S, N, R]] = build[S, N, R](_ => doobie.FC.unit)

}

object DoobieDriverCQRS {
  inline def apply[F[_]: Async](
      inline namespace: String,
      pool: Transactor[F]
  ): F[DoobieDriverCQRS[F]] =
    from(PGNamespace(namespace), pool)

  def from[F[_]: Async](
      namespace: PGNamespace,
      pool: Transactor[F]
  ): F[DoobieDriverCQRS[F]] =
    Queries
      .setupSchema(namespace)
      .run
      .transact(pool)
      .as(new DoobieDriverCQRS(namespace, pool))
}
