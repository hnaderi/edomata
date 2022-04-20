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

import cats.Monad
import cats.effect.Async
import cats.effect.Temporal
import cats.effect.implicits.*
import cats.effect.kernel.Resource
import cats.effect.std.Queue
import cats.implicits.*
import fs2.Chunk
import fs2.Stream

import scala.concurrent.duration.*

trait SnapshotReader[F[_], S] {

  /** Reads snapshot
    *
    * this might involve reading from disk and/or provide the latest version
    * available due to buffering for instance
    * @param id
    *   Stream id to read snapshot for
    * @return
    *   optional snapshot for a folded aggregate
    */
  def get(id: StreamId): F[Option[AggregateState.Valid[S]]]

  /** Reads snapshot from a fast access storage or returns None if not
    * accessible from fast storage/cache. it will always return last version of
    * cache or None.
    *
    * @param id
    *   Stream id to read snapshot for
    * @return
    *   optional snapshot for a folded aggregate
    */
  def getFast(id: StreamId): F[Option[AggregateState.Valid[S]]]
}

trait SnapshotStore[F[_], S] extends SnapshotReader[F, S] {
  def put(id: StreamId, state: AggregateState.Valid[S]): F[Unit]
}

trait SnapshotPersistence[F[_], S] {
  def get(id: StreamId): F[Option[AggregateState.Valid[S]]]
  def put(items: Chunk[SnapshotItem[S]]): F[Unit]
}

object SnapshotStore {
  def inMem[F[_]: Async, S](
      size: Int = 1000
  ): F[SnapshotStore[F, S]] =
    LRUCache[F, StreamId, AggregateState.Valid[S]](size)
      .map(InMemorySnapshotStore(_))

  def persisted[F[_], S](
      store: SnapshotPersistence[F, S],
      size: Int = 1000,
      maxBuffer: Int = 100,
      maxWait: FiniteDuration = 1.minute,
      flushOnExit: Boolean = true
  )(using F: Async[F]): Resource[F, SnapshotStore[F, S]] = for {
    q <- Resource.eval(Queue.dropping[F, SnapshotItem[S]](maxBuffer))
    lc <- Resource.eval(
      LRUCache[F, StreamId, AggregateState.Valid[S]](size)
    )
    pss = PersistedSnapshotStoreImpl(lc, store, q, maxBuffer, maxWait)
    flush = lc.byUsage.use(
      Stream
        .fromIterator(_, size min 1000)
        .chunks
        .evalMap(store.put)
        .compile
        .drain
    )
    persist = Stream
      .fromQueueUnterminated(q, maxBuffer)
      .groupWithin(maxBuffer, maxWait)
      .flatMap(ch => Stream.retry(store.put(ch), 1.second, _ * 2, 3))
    _ <- persist.compile.drain.background
      .onFinalize(if flushOnExit then flush else F.unit)
  } yield pss
}

private final class InMemorySnapshotStore[F[_]: Monad, S](
    cache: LRUCache[F, StreamId, AggregateState.Valid[S]]
) extends SnapshotStore[F, S] {
  def get(id: StreamId): F[Option[AggregateState.Valid[S]]] =
    cache.get(id)
  def getFast(id: StreamId): F[Option[AggregateState.Valid[S]]] = get(id)
  def put(id: StreamId, state: AggregateState.Valid[S]): F[Unit] =
    cache.add(id, state).void
}

type SnapshotItem[S] =
  (StreamId, AggregateState.Valid[S])

private[backend] final class PersistedSnapshotStoreImpl[F[_], S](
    cache: LRUCache[F, StreamId, AggregateState.Valid[S]],
    p: SnapshotPersistence[F, S],
    q: Queue[F, SnapshotItem[S]],
    maxBuffer: Int,
    maxWait: FiniteDuration
)(using F: Temporal[F])
    extends SnapshotStore[F, S] {
  def get(id: StreamId): F[Option[AggregateState.Valid[S]]] =
    getFast(id).flatMap {
      case c @ Some(_) => c.pure
      case None        => p.get(id)
    }
  def put(id: StreamId, state: AggregateState.Valid[S]): F[Unit] =
    cache.add(id, state).flatMap {
      case Some(evicted) => q.tryOffer(evicted).void
      case None          => F.unit
    }
  def getFast(id: StreamId): F[Option[AggregateState.Valid[S]]] =
    cache.get(id)
}
