package edomata.backend

import edomata.core.*

trait CommandStore[F[_], C, M] {
  def append(cmd: CommandMessage[C, M]): F[Unit]
  def contains(id: String): F[Boolean]
}
