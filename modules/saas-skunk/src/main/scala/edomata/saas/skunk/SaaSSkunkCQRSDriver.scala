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
import edomata.backend.PGNaming
import edomata.backend.PGNamespace
import edomata.backend.*
import edomata.backend.cqrs.*
import edomata.core.*
import edomata.saas.TenantExtractor

/** SaaS-aware Skunk CQRS driver that creates tables with `tenant_id` and
  * `owner_id` columns.
  *
  * Drop-in replacement for [[edomata.skunk.SkunkCQRSDriver]] when building
  * multi-tenant SaaS applications.
  *
  * {{{
  * import edomata.saas.skunk.SaaSSkunkCQRSDriver
  *
  * val driver = SaaSSkunkCQRSDriver.from[IO](
  *   PGNaming.prefixed("catalog"),
  *   pool,
  *   skipSetup = false
  * )
  * }}}
  */
final class SaaSSkunkCQRSDriver[F[_]: Async] private (
    naming: PGNaming,
    pool: Resource[F, Session[F]],
    autoSetup: Boolean
) {

  /** Build storage with a notification handler.
    *
    * Requires [[TenantExtractor]] in scope to extract tenant/owner from state.
    */
  def build[S, N, R](
      handler: SkunkHandler[F][N]
  )(using
      tc: StateModelTC[S],
      te: TenantExtractor[S],
      state: BackendCodec[S],
      notifs: BackendCodec[N]
  ): Resource[F, Storage[F, S, N, R]] = {
    def setup = {
      val nQ = SaaSQueries.Outbox(naming, notifs)
      val cQ = SaaSQueries.Commands(naming)
      val sQ = SaaSQueries.State(naming, state)

      val run: F[Unit] =
        if autoSetup then
          pool
            .use(s =>
              s.execute(nQ.setup) >> s.execute(cQ.setup) >> s.execute(sQ.setup)
            )
            .void
        else Async[F].unit
      run.as((nQ, cQ, sQ))
    }

    Resource.eval(setup).flatMap { case (oQ, cQ, sQ) =>
      for {
        updates <- Resource.eval(Notifications[F])
        _outbox = SaaSSkunkOutboxReader(pool, oQ)
        repo = SaaSSkunkCQRSRepository(pool, sQ, oQ, cQ, updates, handler)
      } yield Storage(repo, _outbox, updates)
    }
  }

  /** Build storage without a notification handler. */
  def build[S, N, R](using
      StateModelTC[S],
      TenantExtractor[S],
      BackendCodec[S],
      BackendCodec[N]
  ): Resource[F, Storage[F, S, N, R]] =
    build(_ => _ => Async[F].unit)

  /** Build a complete Backend with optional handler, retry, and caching.
    *
    * This is a convenience method that replaces the
    * `Backend.builder(...).use(driver).build` pipeline, adding
    * [[TenantExtractor]] support.
    */
  def backend[S, N, R](
      handler: Option[SkunkHandler[F][N]] = None,
      maxRetry: Int = 5,
      retryInitialDelay: scala.concurrent.duration.FiniteDuration =
        scala.concurrent.duration.DurationInt(2).seconds
  )(using
      StateModelTC[S],
      TenantExtractor[S],
      BackendCodec[S],
      BackendCodec[N]
  ): Resource[F, Backend[F, S, R, N]] =
    for {
      storage <- handler.fold(build[S, N, R])(build[S, N, R](_))
      given cats.effect.std.Random[F] <- Resource.eval(
        cats.effect.std.Random.scalaUtilRandom[F]
      )
    } yield new Backend[F, S, R, N] {
      override def compile: CommandHandler[F, S, N] =
        CommandHandler.withRetry(
          storage.repository,
          maxRetry,
          retryInitialDelay
        )
      override def outbox: OutboxReader[F, N] = storage.outbox
      override def repository: RepositoryReader[F, S] = storage.repository
      override def updates: NotificationsConsumer[F] = storage.updates
    }
}

object SaaSSkunkCQRSDriver {
  inline def apply[F[_]: Async](
      inline namespace: String,
      pool: Resource[F, Session[F]]
  ): F[SaaSSkunkCQRSDriver[F]] = from(PGNamespace(namespace), pool)

  def from[F[_]: Async](
      namespace: PGNamespace,
      pool: Resource[F, Session[F]]
  ): F[SaaSSkunkCQRSDriver[F]] = from(PGNaming.schema(namespace), pool)

  def from[F[_]: Async](
      naming: PGNaming,
      pool: Resource[F, Session[F]],
      skipSetup: Boolean = false
  ): F[SaaSSkunkCQRSDriver[F]] =
    val setup: F[Unit] =
      if !skipSetup && naming.needsSchemaSetup then
        pool.use(_.execute(SaaSQueries.setupSchema(naming.namespace))).void
      else Async[F].unit
    setup.as(new SaaSSkunkCQRSDriver(naming, pool, autoSetup = !skipSetup))
}
