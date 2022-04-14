package edomata.backend

import cats.effect.Async
import cats.effect.Concurrent
import cats.effect.kernel.Ref
import cats.implicits.*

trait SnapshotStore[F[_], S, E, R] {
  def get(id: StreamId): F[Option[AggregateState.Valid[S, E, R]]]
  def put(id: StreamId, state: AggregateState.Valid[S, E, R]): F[Unit]
}

object SnapshotStore {
  def inMem[F[_]: Async, S, E, R](
      size: Int = 1000
  ): F[SnapshotStore[F, S, E, R]] =
    LRUCache[F, StreamId, AggregateState.Valid[S, E, R]](size)
      .map(InMemorySnapshotStore(_))
}

private final class InMemorySnapshotStore[F[_]: Concurrent, S, E, R](
    cache: LRUCache[F, StreamId, AggregateState.Valid[S, E, R]]
) extends SnapshotStore[F, S, E, R] {
  def get(id: StreamId): F[Option[AggregateState.Valid[S, E, R]]] =
    cache.get(id)
  def put(id: StreamId, state: AggregateState.Valid[S, E, R]): F[Unit] =
    cache.add(id, state).void
}
