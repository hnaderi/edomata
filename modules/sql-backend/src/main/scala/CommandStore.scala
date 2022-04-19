package edomata.backend

import cats.Monad
import cats.effect.Concurrent
import cats.effect.kernel.Async
import cats.effect.kernel.Ref
import cats.implicits.*
import edomata.core.*

import scala.collection.immutable.HashSet

trait CommandStore[F[_]] {
  def append(cmd: CommandMessage[?]): F[Unit]
  def contains(id: String): F[Boolean]
}

object CommandStore {
  def inMem[F[_]](size: Int)(using F: Async[F]): F[CommandStore[F]] =
    (LRUCache[F, String, Unit](size), F.ref(HashSet.empty[String]))
      .mapN(InMemoryCommandStore(_, _))

  private[backend] final class InMemoryCommandStore[F[_]: Monad](
      cache: Cache[F, String, Unit],
      set: Ref[F, HashSet[String]]
  ) extends CommandStore[F] {
    def append(cmd: CommandMessage[?]): F[Unit] =
      cache.add(cmd.id, ()).flatMap {
        case Some((id, _)) => set.update(_ - id + cmd.id)
        case None          => set.update(_ + cmd.id)
      }
    def contains(id: String): F[Boolean] = set.get.map(_.contains(id))
  }
}
