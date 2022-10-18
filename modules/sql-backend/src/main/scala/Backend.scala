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

import cats.effect.kernel.Async
import cats.effect.kernel.Resource
import edomata.core.*

import scala.concurrent.duration.*

trait Backend[F[_], S, E, R, N] {
  def compile: CommandHandler[F, S, E, R, N]
  def outbox: OutboxReader[F, N]
  def journal: JournalReader[F, E]
  def repository: RepositoryReader[F, S, E, R]
  def updates: NotificationsConsumer[F]
}

object Backend {
  final case class Builder[
      F[_]: Async,
      Codec[_],
      C,
      S,
      E,
      R,
      N
  ] private[backend] (
      private val driver: StorageDriver[F, Codec, S, E, R, N],
      private val domain: Domain[C, S, E, R, N],
      private val snapshot: Resource[F, SnapshotStore[F, S]],
      val maxRetry: Int = 5,
      val retryInitialDelay: FiniteDuration = 2.seconds,
      val cached: Boolean = true
  )(using ModelTC[S, E, R]) {

    def persistedSnapshot(
        maxInMem: Int = 1000,
        maxBuffer: Int = 100,
        maxWait: FiniteDuration = 1.minute
    )(using codec: Codec[S]): Builder[F, Codec, C, S, E, R, N] =
      copy(snapshot =
        driver.snapshot
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

    def disableCache: Builder[F, Codec, C, S, E, R, N] = copy(cached = false)

    def inMemSnapshot(
        maxInMem: Int = 1000
    ): Builder[F, Codec, C, S, E, R, N] =
      copy(snapshot = Resource.eval(SnapshotStore.inMem(maxInMem)))

    def withSnapshot(
        s: Resource[F, SnapshotStore[F, S]]
    ): Builder[F, Codec, C, S, E, R, N] = copy(snapshot = s)

    def withRetryConfig(
        maxRetry: Int = maxRetry,
        retryInitialDelay: FiniteDuration = retryInitialDelay
    ): Builder[F, Codec, C, S, E, R, N] =
      copy(maxRetry = maxRetry, retryInitialDelay = retryInitialDelay)

    def build(using
        event: Codec[E],
        notifs: Codec[N]
    ): Resource[F, Backend[F, S, E, R, N]] = for {
      s <- snapshot
      storage <- driver.build(s)
      compiler <-
        if cached then
          Resource
            .eval(CommandStore.inMem(100))
            .map(CachedRepository(storage.repository, _, s))
        else Resource.pure(storage.repository)
      h = CommandHandler.withRetry(compiler, maxRetry, retryInitialDelay)

    } yield BackendImpl(
      h,
      storage.outbox,
      storage.journal,
      storage.reader,
      storage.updates
    )
  }

  final class PartialBuilder[C, S, E, R, N](
      domain: Domain[C, S, E, R, N]
  )(using model: ModelTC[S, E, R]) {
    def using[F[_]: Async, Codec[_]](
        driver: StorageDriver[F, Codec, S, E, R, N]
    ): Builder[F, Codec, C, S, E, R, N] =
      Builder(driver, domain, Resource.eval(SnapshotStore.inMem(1000)))
  }

  def builder[C, S, E, R, N](
      domain: Domain[C, S, E, R, N]
  )(using
      model: ModelTC[S, E, R]
  ): PartialBuilder[C, S, E, R, N] = new PartialBuilder(domain)
  def builder[C, S, E, R, N](
      service: edomata.core.DomainModel[S, E, R]#Service[C, N]
  )(using
      model: ModelTC[S, E, R]
  ): PartialBuilder[C, S, E, R, N] = new PartialBuilder(service.domain)
}

private[edomata] final case class BackendImpl[F[_], S, E, R, N](
    compile: CommandHandler[F, S, E, R, N],
    outbox: OutboxReader[F, N],
    journal: JournalReader[F, E],
    repository: RepositoryReader[F, S, E, R],
    updates: NotificationsConsumer[F]
) extends Backend[F, S, E, R, N]

final case class Storage[F[_], S, E, R, N](
    repository: Repository[F, S, E, R, N],
    reader: RepositoryReader[F, S, E, R],
    journal: JournalReader[F, E],
    outbox: OutboxReader[F, N],
    updates: NotificationsConsumer[F]
)

trait StorageDriver[F[_], Codec[_], S, E, R, N] {
  def build(snapshot: SnapshotStore[F, S])(using
      ModelTC[S, E, R],
      Codec[E],
      Codec[N]
  ): Resource[F, Storage[F, S, E, R, N]]
  def snapshot(using Codec[S]): Resource[F, SnapshotPersistence[F, S]]
}
