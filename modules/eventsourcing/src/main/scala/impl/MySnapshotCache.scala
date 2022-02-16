package edomata.eventsourcing

import cats.Show
import cats.effect.kernel.Async
import cats.effect.kernel.Resource
import cats.implicits.*
import fs2.Stream
import io.odin.Logger
import io.odin.consoleLogger

object MySnapshotCache {
  def apply[F[_]: Async, I: Show, S](
      store: SnapshotStore[F, I, S],
      size: Int = 1000
  ): Resource[F, SnapshotCache[F, I, S]] =
    withLogger(store, consoleLogger(), size)

  def withLogger[F[_]: Async, I: Show, S](
      store: SnapshotStore[F, I, S],
      logger: Logger[F],
      size: Int = 1000
  ): Resource[F, SnapshotCache[F, I, S]] = {
    val acquire = LRUCache[F, I, S](size)
    def release(cache: LRUCache[F, I, S]) =
      logger.debug("Persisting cache...") >>
        Stream
          .fromIterator(cache.iterator, size)
          .evalMap(store.put(_, _))
          .onFinalize(logger.debug("Cache persisted."))
          .compile
          .drain

    val build = Resource.make(acquire)(release)
    build.map { cache =>
      new SnapshotCache[F, I, S] {
        def get(id: I): F[Option[S]] = cache.get(id).flatMap {
          case cached @ Some(_) =>
            logger.trace("Loaded from cache!", Map("id" -> id.show)).as(cached)
          case None =>
            logger.info(
              "Cache miss! reading from storage...",
              Map("id" -> id.show)
            ) >> store.get(id)
        }
        def put(id: I, state: S): F[Unit] = cache.add(id, state).void
      }
    }
  }
}
