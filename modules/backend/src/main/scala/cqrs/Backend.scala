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
    snapshot: StorageDriver[F, Codec] => Resource[F, SnapshotStore[F, S]],
    commandCache: Option[Resource[F, CommandStore[F]]],
    val maxRetry: Int = 5,
    val retryInitialDelay: FiniteDuration = 2.seconds
)(using StateModelTC[S]) {
  private def copy(
      driver: Resource[F, StorageDriver[F, Codec]] = driver,
      domain: CQRSDomain[C, S, R, N] = domain,
      snapshot: StorageDriver[F, Codec] => Resource[F, SnapshotStore[F, S]] =
        snapshot,
      commandCache: Option[Resource[F, CommandStore[F]]] = commandCache,
      maxRetry: Int = maxRetry,
      retryInitialDelay: FiniteDuration = retryInitialDelay
  ) = new BackendBuilder(
    driver = driver,
    domain = domain,
    snapshot = snapshot,
    maxRetry = maxRetry,
    commandCache = commandCache,
    retryInitialDelay = retryInitialDelay
  )

  def disableCache: BackendBuilder[F, Codec, C, S, R, N] = ???

  def withCommandCache(
      cache: Resource[F, CommandStore[F]]
  ): BackendBuilder[F, Codec, C, S, R, N] = ???

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
  ): BackendBuilder[F, Codec, C, S, R, N] = ???

  def inMemSnapshot(
      maxInMem: Int = 1000
  ): BackendBuilder[F, Codec, C, S, R, N] =
    ???

  def withSnapshot(
      s: Resource[F, SnapshotStore[F, S]]
  ): BackendBuilder[F, Codec, C, S, R, N] = ???

  def withRetryConfig(
      maxRetry: Int = maxRetry,
      retryInitialDelay: FiniteDuration = retryInitialDelay
  ): BackendBuilder[F, Codec, C, S, R, N] =
    ???

  def build(using
      state: Codec[S],
      notifs: Codec[N]
  ): Resource[F, Backend[F, S, R, N]] = ???
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
      snapshot = _ => Resource.eval(SnapshotStore.inMem(1000)),
      commandCache = Some(Resource.eval(CommandStore.inMem(1000)))
    )
}
