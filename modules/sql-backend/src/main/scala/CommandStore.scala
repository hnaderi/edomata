package edomata.backend

import edomata.core.*

trait CommandStore[F[_], C] {
  def append(cmd: CommandMessage[C]): F[Unit]
  def contains(id: String): F[Boolean]
}
