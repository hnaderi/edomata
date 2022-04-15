package edomata.backend

import cats.Monad
import cats.effect.Async
import cats.effect.implicits.*
import cats.effect.kernel.Resource
import cats.effect.std.Queue
import cats.implicits.*
import fs2.Chunk

import scala.concurrent.duration.*

trait SnapshotStore[F[_], S, E, R] {
  def get(id: StreamId): F[Option[AggregateState.Valid[S, E, R]]]
  def put(id: StreamId, state: AggregateState.Valid[S, E, R]): F[Unit]
}

trait SnapshotPersistence[F[_], S, E, R] {
  def get(id: StreamId): F[Option[AggregateState.Valid[S, E, R]]]
  def put(items: Chunk[SnapshotItem[S, E, R]]): F[Unit]
}

object SnapshotStore {
  def inMem[F[_]: Async, S, E, R](
      size: Int = 1000
  ): F[SnapshotStore[F, S, E, R]] =
    LRUCache[F, StreamId, AggregateState.Valid[S, E, R]](size)
      .map(InMemorySnapshotStore(_))

  def persisted[F[_]: Async, S, E, R](
      store: SnapshotPersistence[F, S, E, R],
      size: Int = 1000,
      maxBuffer: Int = 100,
      maxWait: FiniteDuration = 1.minute
  ): Resource[F, SnapshotStore[F, S, E, R]] = for {
    q <- Resource.eval(Queue.dropping[F, SnapshotItem[S, E, R]](maxBuffer))
    lc <- Resource.eval(
      LRUCache[F, StreamId, AggregateState.Valid[S, E, R]](size)
    )
    pss = PersistedSnapshotStore(lc, store, q)
    _ <- fs2.Stream
      .fromQueueUnterminated(q, maxBuffer)
      .groupWithin(maxBuffer, maxWait)
      .evalMap(store.put)
      .compile
      .drain
      .background
  } yield pss
}

private final class InMemorySnapshotStore[F[_]: Monad, S, E, R](
    cache: LRUCache[F, StreamId, AggregateState.Valid[S, E, R]]
) extends SnapshotStore[F, S, E, R] {
  def get(id: StreamId): F[Option[AggregateState.Valid[S, E, R]]] =
    cache.get(id)
  def put(id: StreamId, state: AggregateState.Valid[S, E, R]): F[Unit] =
    cache.add(id, state).void
}

type SnapshotItem[S, E, R] =
  (StreamId, AggregateState.Valid[S, E, R])

private final class PersistedSnapshotStore[F[_], S, E, R](
    cache: LRUCache[F, StreamId, AggregateState.Valid[S, E, R]],
    p: SnapshotPersistence[F, S, E, R],
    q: Queue[F, SnapshotItem[S, E, R]]
)(using F: Monad[F])
    extends SnapshotStore[F, S, E, R] {
  def get(id: StreamId): F[Option[AggregateState.Valid[S, E, R]]] =
    cache.get(id).flatMap {
      case c @ Some(_) => c.pure
      case None        => p.get(id)
    }
  def put(id: StreamId, state: AggregateState.Valid[S, E, R]): F[Unit] =
    cache.add(id, state).flatMap {
      case Some(evicted) => q.offer(evicted)
      case None          => F.unit
    }
}
