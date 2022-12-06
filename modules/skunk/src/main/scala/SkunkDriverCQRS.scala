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
import cats.data.NonEmptyChain
import cats.effect.kernel.Async
import cats.effect.kernel.Resource
import cats.implicits.*
import edomata.backend.PGNamespace
import edomata.backend.cqrs.*
import edomata.core.*

final class SkunkDriverCQRS[F[_]: Async] private (
    namespace: PGNamespace,
    pool: Resource[F, Session[F]]
) extends StorageDriver[F, BackendCodec, SkunkHandler[F]] {

  override def build[S, N, R](
      handler: SkunkHandler[F][N]
  )(using
      tc: StateModelTC[S],
      state: BackendCodec[S],
      notifs: BackendCodec[N]
  ): Resource[F, Storage[F, S, N, R]] = {
    def setup = {
      val nQ = Queries.Outbox(namespace, notifs)
      val cQ = Queries.Commands(namespace)
      val sQ = Queries.State(namespace, state)

      pool
        .use(s =>
          s.execute(nQ.setup) >> s.execute(cQ.setup) >> s.execute(sQ.setup)
        )
        .as((nQ, cQ, sQ))
    }

    Resource.eval(setup).flatMap { case (oQ, cQ, sQ) =>
      for {
        updates <- Resource.eval(Notifications[F])
        _outbox = SkunkOutboxReader(pool, oQ)
        repo = SkunkCQRSRepository(pool, sQ, oQ, cQ, updates, handler)
      } yield Storage(repo, _outbox, updates)
    }

  }

  override def build[S, N, R](using
      StateModelTC[S],
      BackendCodec[S],
      BackendCodec[N]
  ): Resource[F, Storage[F, S, N, R]] =
    build(_ => _ => Async[F].unit)

}

object SkunkDriverCQRS {
  inline def apply[F[_]: Async](
      inline namespace: String,
      pool: Resource[F, Session[F]]
  ): F[SkunkDriverCQRS[F]] = from(PGNamespace(namespace), pool)

  def from[F[_]: Async](
      namespace: PGNamespace,
      pool: Resource[F, Session[F]]
  ): F[SkunkDriverCQRS[F]] =
    pool
      .use(_.execute(Queries.setupSchema(namespace)))
      .as(new SkunkDriverCQRS(namespace, pool))
}
