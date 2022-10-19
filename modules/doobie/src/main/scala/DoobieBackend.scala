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

import _root_.doobie.ConnectionIO
import _root_.doobie.FC
import _root_.doobie.Transactor
import _root_.doobie.implicits.*
import cats.data.Chain
import cats.data.EitherNec
import cats.data.NonEmptyChain
import cats.effect.Concurrent
import cats.effect.Temporal
import cats.effect.kernel.Async
import cats.effect.kernel.Clock
import cats.effect.kernel.Resource
import cats.implicits.*
import edomata.backend.*
import edomata.core.*
import fs2.Stream

import java.time.OffsetDateTime
import java.time.ZoneOffset
import scala.concurrent.duration.*

@deprecated("Use edomata.Backend instead", "0.6.0")
object DoobieBackend {

  def apply[F[_]: Async](trx: Transactor[F]): PartialBuilder[F] =
    PartialBuilder(trx)

  final class PartialBuilder[F[_]: Async](trx: Transactor[F]) {

    inline def builder[C, S, E, R, N](
        domain: Domain[C, S, E, R, N],
        inline namespace: String
    )(using
        model: ModelTC[S, E, R]
    ): DomainBuilder[F, C, S, E, R, N] =
      val ns = PGNamespace(namespace)
      DomainBuilder(
        Backend.builder(domain).use(DoobieDriver.from(ns, trx)),
        ns
      )

    inline def builder[C, S, E, R, N](
        service: edomata.core.DomainModel[S, E, R]#Service[C, N],
        inline namespace: String
    )(using
        model: ModelTC[S, E, R]
    ): DomainBuilder[F, C, S, E, R, N] =
      val ns = PGNamespace(namespace)
      DomainBuilder(
        Backend.builder(service.domain).use(DoobieDriver.from(ns, trx)),
        ns
      )

  }

  final case class DomainBuilder[
      F[_]: Async,
      C,
      S,
      E,
      R,
      N
  ] private[doobie] (
      private val builder: Backend.Builder[F, BackendCodec, C, S, E, R, N],
      val namespace: PGNamespace
  ) {
    export builder.{maxRetry, retryInitialDelay, cached}

    def persistedSnapshot(
        maxInMem: Int = 1000,
        maxBuffer: Int = 100,
        maxWait: FiniteDuration = 1.minute
    )(using codec: BackendCodec[S]): DomainBuilder[F, C, S, E, R, N] =
      copy(builder = builder.persistedSnapshot(maxInMem, maxBuffer, maxWait))

    def disableCache: DomainBuilder[F, C, S, E, R, N] =
      copy(builder = builder.disableCache)

    def inMemSnapshot(
        maxInMem: Int = 1000
    ): DomainBuilder[F, C, S, E, R, N] =
      copy(builder = builder.inMemSnapshot(maxInMem))

    def withSnapshot(
        s: Resource[F, SnapshotStore[F, S]]
    ): DomainBuilder[F, C, S, E, R, N] = copy(builder = builder.withSnapshot(s))

    def withRetryConfig(
        maxRetry: Int = maxRetry,
        retryInitialDelay: FiniteDuration = retryInitialDelay
    ): DomainBuilder[F, C, S, E, R, N] =
      copy(builder = builder.withRetryConfig(maxRetry, retryInitialDelay))

    def build(using
        event: BackendCodec[E],
        notifs: BackendCodec[N]
    ): Resource[F, Backend[F, S, E, R, N]] = builder.build
  }
}
