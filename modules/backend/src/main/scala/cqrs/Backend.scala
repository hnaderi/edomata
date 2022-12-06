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

package edomata.backend
package cqrs

import cats.effect.kernel.Async
import cats.effect.kernel.Resource
import edomata.core.*

import scala.concurrent.duration.*

trait Backend[F[_], S, R, N] {
  def compile: CommandHandler[F, S, N]
  def outbox: OutboxReader[F, N]
  def repository: RepositoryReader[F, S]
  def updates: NotificationsConsumer[F]
}

final class BackendBuilder[F[_]: Async, Codec[_], C, S, R, N] private[cqrs] (
    driver: Resource[F, StorageDriver[F, Codec]],
    domain: CQRSDomain[C, S, R, N],
    commandCache: Option[Resource[F, CommandStore[F]]],
    val maxRetry: Int = 5,
    val retryInitialDelay: FiniteDuration = 2.seconds
)(using StateModelTC[S]) {
  private def copy(
      driver: Resource[F, StorageDriver[F, Codec]] = driver,
      domain: CQRSDomain[C, S, R, N] = domain,
      commandCache: Option[Resource[F, CommandStore[F]]] = commandCache,
      maxRetry: Int = maxRetry,
      retryInitialDelay: FiniteDuration = retryInitialDelay
  ) = new BackendBuilder(
    driver = driver,
    domain = domain,
    maxRetry = maxRetry,
    commandCache = commandCache,
    retryInitialDelay = retryInitialDelay
  )

  def disableCache: BackendBuilder[F, Codec, C, S, R, N] =
    copy(commandCache = None)

  def withCommandCache(
      cache: Resource[F, CommandStore[F]]
  ): BackendBuilder[F, Codec, C, S, R, N] = copy(commandCache = Some(cache))

  def withCommandCache(
      cache: CommandStore[F]
  ): BackendBuilder[F, Codec, C, S, R, N] = withCommandCache(
    Resource.pure(cache)
  )

  def withCommandCache(
      cache: F[CommandStore[F]]
  ): BackendBuilder[F, Codec, C, S, R, N] = withCommandCache(
    Resource.eval(cache)
  )

  def withCommandCacheSize(
      maxCommandsToCache: Int
  ): BackendBuilder[F, Codec, C, S, R, N] = copy(commandCache =
    Some(Resource.eval(CommandStore.inMem(maxCommandsToCache)))
  )

  def withRetryConfig(
      maxRetry: Int = maxRetry,
      retryInitialDelay: FiniteDuration = retryInitialDelay
  ): BackendBuilder[F, Codec, C, S, R, N] =
    copy(maxRetry = maxRetry, retryInitialDelay = retryInitialDelay)

  def build(using
      state: Codec[S],
      notifs: Codec[N]
  ): Resource[F, Backend[F, S, R, N]] = for {
    d <- driver
    storage <- d.build[S, N, R]
  } yield new Backend[F, S, R, N] {

    override def compile: CommandHandler[F, S, N] =
      CommandHandler.withRetry(storage.repository, maxRetry, retryInitialDelay)

    override def outbox: OutboxReader[F, N] = storage.outbox

    override def repository: RepositoryReader[F, S] = storage.repository

    override def updates: NotificationsConsumer[F] = storage.updates

  }
}

final class PartialBackendBuilder[C, S, R, N] private[backend] (
    domain: CQRSDomain[C, S, R, N]
)(using model: StateModelTC[S]) {
  def use[F[_]: Async, Codec[_]](
      driver: StorageDriver[F, Codec]
  ): BackendBuilder[F, Codec, C, S, R, N] =
    from(Resource.pure(driver))

  def use[F[_]: Async, Codec[_], D <: StorageDriver[F, Codec]](
      driver: F[D]
  ): BackendBuilder[F, Codec, C, S, R, N] =
    from(Resource.eval(driver))

  def from[F[_]: Async, Codec[_], D <: StorageDriver[F, Codec]](
      driver: Resource[F, D]
  ): BackendBuilder[F, Codec, C, S, R, N] =
    new BackendBuilder(
      driver,
      domain,
      commandCache = Some(Resource.eval(CommandStore.inMem(1000)))
    )
}
