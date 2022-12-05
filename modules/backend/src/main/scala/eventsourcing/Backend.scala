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
package eventsourcing

trait Backend[F[_], S, E, R, N] {
  def compile: CommandHandler[F, S, E, R, N]
  def outbox: OutboxReader[F, N]
  def journal: JournalReader[F, E]
  def repository: RepositoryReader[F, S, E, R]
  def updates: NotificationsConsumer[F]
}

import cats.effect.kernel.Async
import cats.effect.kernel.Resource
import edomata.core.*

import scala.concurrent.duration.*
final class BackendBuilder[
    F[_]: Async,
    Codec[_],
    C,
    S,
    E,
    R,
    N
] private[eventsourcing] (
    driver: Resource[F, StorageDriver[F, Codec]],
    domain: Domain[C, S, E, R, N],
    snapshot: StorageDriver[F, Codec] => Resource[F, SnapshotStore[F, S]],
    commandCache: Option[Resource[F, CommandStore[F]]],
    val maxRetry: Int = 5,
    val retryInitialDelay: FiniteDuration = 2.seconds
)(using ModelTC[S, E, R]) {
  private def copy(
      driver: Resource[F, StorageDriver[F, Codec]] = driver,
      domain: Domain[C, S, E, R, N] = domain,
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

  def persistedSnapshot(
      maxInMem: Int = 1000,
      maxBuffer: Int = 100,
      maxWait: FiniteDuration = 1.minute
  )(using codec: Codec[S]): BackendBuilder[F, Codec, C, S, E, R, N] =
    copy(snapshot =
      _.snapshot
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

  def disableCache: BackendBuilder[F, Codec, C, S, E, R, N] =
    copy(commandCache = None)
  def withCommandCache(
      cache: Resource[F, CommandStore[F]]
  ): BackendBuilder[F, Codec, C, S, E, R, N] = copy(commandCache = Some(cache))
  def withCommandCache(
      cache: CommandStore[F]
  ): BackendBuilder[F, Codec, C, S, E, R, N] = withCommandCache(
    Resource.pure(cache)
  )
  def withCommandCache(
      cache: F[CommandStore[F]]
  ): BackendBuilder[F, Codec, C, S, E, R, N] = withCommandCache(
    Resource.eval(cache)
  )
  def withCommandCacheSize(
      maxCommandsToCache: Int
  ): BackendBuilder[F, Codec, C, S, E, R, N] =
    copy(commandCache =
      Some(Resource.eval(CommandStore.inMem(maxCommandsToCache)))
    )

  def inMemSnapshot(
      maxInMem: Int = 1000
  ): BackendBuilder[F, Codec, C, S, E, R, N] =
    copy(snapshot = _ => Resource.eval(SnapshotStore.inMem(maxInMem)))

  def withSnapshot(
      s: Resource[F, SnapshotStore[F, S]]
  ): BackendBuilder[F, Codec, C, S, E, R, N] = copy(snapshot = _ => s)

  def withRetryConfig(
      maxRetry: Int = maxRetry,
      retryInitialDelay: FiniteDuration = retryInitialDelay
  ): BackendBuilder[F, Codec, C, S, E, R, N] =
    copy(maxRetry = maxRetry, retryInitialDelay = retryInitialDelay)

  def build(using
      event: Codec[E],
      notifs: Codec[N]
  ): Resource[F, Backend[F, S, E, R, N]] = for {
    dr <- driver
    s <- snapshot(dr)
    storage <- dr.build[S, E, R, N](s)
    compiler <- commandCache.fold(Resource.pure(storage.repository))(
      _.map(CachedRepository(storage.repository, _, s))
    )
    h = CommandHandler.withRetry(compiler, maxRetry, retryInitialDelay)

  } yield BackendImpl(
    h,
    storage.outbox,
    storage.journal,
    storage.reader,
    storage.updates
  )
}

final class PartialBackendBuilder[C, S, E, R, N] private[backend] (
    domain: Domain[C, S, E, R, N]
)(using model: ModelTC[S, E, R]) {
  def use[F[_]: Async, Codec[_]](
      driver: StorageDriver[F, Codec]
  ): BackendBuilder[F, Codec, C, S, E, R, N] =
    from(Resource.pure(driver))

  def use[F[_]: Async, Codec[_], D <: StorageDriver[F, Codec]](
      driver: F[D]
  ): BackendBuilder[F, Codec, C, S, E, R, N] =
    from(Resource.eval(driver))

  def from[F[_]: Async, Codec[_], D <: StorageDriver[F, Codec]](
      driver: Resource[F, D]
  ): BackendBuilder[F, Codec, C, S, E, R, N] =
    new BackendBuilder(
      driver,
      domain,
      snapshot = _ => Resource.eval(SnapshotStore.inMem(1000)),
      commandCache = Some(Resource.eval(CommandStore.inMem(1000)))
    )
}
private final case class BackendImpl[F[_], S, E, R, N](
    compile: CommandHandler[F, S, E, R, N],
    outbox: OutboxReader[F, N],
    journal: JournalReader[F, E],
    repository: RepositoryReader[F, S, E, R],
    updates: NotificationsConsumer[F]
) extends Backend[F, S, E, R, N]
