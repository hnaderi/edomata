package edomata.backend

import cats.effect.kernel.Async
import cats.implicits.*

trait Cache[F[_], I, T] {
  def add(key: I, value: T): F[Option[(I, T)]]
  def get(key: I): F[Option[T]]
}

object Cache {
  def lru[F[_]: Async, Id, State](size: Int): F[Cache[F, Id, State]] =
    LRUCache(size).widen
}
