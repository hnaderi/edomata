package edomata.backend

import cats.data.NonEmptyChain
import edomata.core.*

trait Repository[F[_], S, E, R, N] {
  def load(cmd: CommandMessage[?]): F[CommandState[S, E, R]]

  def append(
      ctx: RequestContext[?, ?],
      version: SeqNr,
      newState: S,
      events: NonEmptyChain[E],
      notifications: Seq[N]
  ): F[Unit]

  def notify(
      ctx: RequestContext[?, ?],
      notifications: NonEmptyChain[N]
  ): F[Unit]
}

type CommandState[S, E, R] =
  AggregateState[S, E, R] | CommandState.Redundant.type
object CommandState {
  case object Redundant
}
