package edfsm.backend

import edfsm.backend.CommandMessage

trait CommandStore[F[_], T] {
  def append(cmd: CommandMessage[T]): F[Unit]
  def contains(id: String): F[Boolean]
}
