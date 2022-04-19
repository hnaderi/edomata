package edomata.backend

import cats.effect.Concurrent
import cats.effect.kernel.Async
import cats.implicits.*
import edomata.core.*

import scala.collection.immutable.HashSet

trait CommandStore[F[_]] {
  def append(cmd: CommandMessage[?]): F[Unit]
  def contains(id: String): F[Boolean]
}

object CommandStore {
  def inMem[F[_]](size: Int)(using F: Async[F]): F[CommandStore[F]] = for {
    c <- LRUCache[F, String, Unit](size)
    s <- F.ref(HashSet.empty[String])
  } yield new {

    def append(cmd: CommandMessage[?]): F[Unit] = c.add(cmd.id, ()).flatMap {
      case Some((id, _)) => s.update(_ - id + cmd.id)
      case None          => s.update(_ + cmd.id)
    }
    def contains(id: String): F[Boolean] = s.get.map(_.contains(id))
  }
}
