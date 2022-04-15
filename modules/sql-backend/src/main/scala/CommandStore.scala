package edomata.backend

import edomata.core.*

trait CommandStore[F[_]] {
  def append(cmd: CommandMessage[?]): F[Unit]
  def contains(id: String): F[Boolean]
}
