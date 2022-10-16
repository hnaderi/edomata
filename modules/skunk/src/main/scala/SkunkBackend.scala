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

import _root_.skunk.Codec
import _root_.skunk.Session
import _root_.skunk.data.Identifier
import cats.data.EitherNec
import cats.data.NonEmptyChain
import cats.effect.Concurrent
import cats.effect.implicits.*
import cats.effect.kernel.Async
import cats.effect.kernel.Clock
import cats.effect.kernel.Resource
import cats.effect.kernel.Temporal
import cats.implicits.*
import edomata.backend.*
import edomata.core.*
import fs2.Stream

import java.time.OffsetDateTime
import java.time.ZoneOffset
import scala.concurrent.duration.*

object SkunkBackend {
  def apply[F[_]: Async](pool: Resource[F, Session[F]]): PartialBuilder[F] =
    PartialBuilder(pool)

  final class PartialBuilder[F[_]: Async](pool: Resource[F, Session[F]]) {
    inline def builder[C, S, E, R, N](
        domain: Domain[C, S, E, R, N],
        inline namespace: String
    )(using
        model: ModelTC[S, E, R]
    ): DomainBuilder[F, C, S, E, R, N] =
      DomainBuilder(
        pool,
        domain,
        model,
        PGNamespace(namespace),
        Resource.eval(SnapshotStore.inMem(1000))
      )

    inline def builder[C, S, E, R, N](
        service: edomata.core.DomainModel[S, E, R]#Service[C, N],
        inline namespace: String
    )(using
        model: ModelTC[S, E, R]
    ): DomainBuilder[F, C, S, E, R, N] =
      DomainBuilder(
        pool,
        service.domain,
        model,
        PGNamespace(namespace),
        Resource.eval(SnapshotStore.inMem(1000))
      )
  }

  final case class DomainBuilder[
      F[_]: Async,
      C,
      S,
      E,
      R,
      N
  ] private[skunk] (
      private val pool: Resource[F, Session[F]],
      private val domain: Domain[C, S, E, R, N],
      private val model: ModelTC[S, E, R],
      val namespace: PGNamespace,
      private val snapshot: Resource[F, SnapshotStore[F, S]],
      val maxRetry: Int = 5,
      val retryInitialDelay: FiniteDuration = 2.seconds,
      val cached: Boolean = true
  ) {

    def persistedSnapshot(
        maxInMem: Int = 1000,
        maxBuffer: Int = 100,
        maxWait: FiniteDuration = 1.minute
    )(using codec: BackendCodec[S]): DomainBuilder[F, C, S, E, R, N] =
      copy(snapshot =
        Resource
          .eval(SkunkSnapshotPersistence(pool, namespace))
          .flatMap(store =>
            SnapshotStore
              .persisted(
                store,
                size = maxInMem,
                maxBuffer = maxBuffer,
                maxWait
              )
          )
      )

    def disableCache: DomainBuilder[F, C, S, E, R, N] = copy(cached = false)

    def inMemSnapshot(
        maxInMem: Int = 1000
    ): DomainBuilder[F, C, S, E, R, N] =
      copy(snapshot = Resource.eval(SnapshotStore.inMem(maxInMem)))

    def withSnapshot(
        s: Resource[F, SnapshotStore[F, S]]
    ): DomainBuilder[F, C, S, E, R, N] = copy(snapshot = s)

    def withRetryConfig(
        maxRetry: Int = maxRetry,
        retryInitialDelay: FiniteDuration = retryInitialDelay
    ): DomainBuilder[F, C, S, E, R, N] =
      copy(maxRetry = maxRetry, retryInitialDelay = retryInitialDelay)

    private def _setup(using
        event: BackendCodec[E],
        notifs: BackendCodec[N]
    ) = {
      val jQ = Queries.Journal(namespace, event)
      val nQ = Queries.Outbox(namespace, notifs)
      val cQ = Queries.Commands(namespace)

      pool
        .use(s =>
          s.execute(Queries.setupSchema(namespace)) >>
            s.execute(jQ.setup) >> s.execute(nQ.setup) >> s.execute(cQ.setup)
        )
        .as((jQ, nQ, cQ))
    }

    def setup(using
        event: BackendCodec[E],
        notifs: BackendCodec[N]
    ): F[Unit] = _setup.void

    def build(using
        event: BackendCodec[E],
        notifs: BackendCodec[N]
    ): Resource[F, Backend[F, S, E, R, N]] = for {
      qs <- Resource.eval(_setup)
      (jQ, nQ, cQ) = qs
      given ModelTC[S, E, R] = model
      s <- snapshot
      updates <- Resource.eval(Notifications[F])
      _outbox = SkunkOutboxReader(pool, nQ)
      _journal = SkunkJournalReader(pool, jQ)
      _repo = RepositoryReader(_journal, s)
      skRepo = SkunkRepository(pool, jQ, nQ, cQ, _repo, updates)
      compiler <-
        if cached then
          Resource
            .eval(CommandStore.inMem(100))
            .map(CachedRepository(skRepo, _, s))
        else Resource.pure(skRepo)
      h = CommandHandler.withRetry(compiler, maxRetry, retryInitialDelay)

    } yield BackendImpl(h, _outbox, _journal, _repo, updates)
  }
}
