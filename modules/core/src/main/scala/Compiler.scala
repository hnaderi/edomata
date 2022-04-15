package edomata.core

import cats.Monad
import cats.data.EitherNec
import cats.data.NonEmptyChain
import cats.implicits.*

import java.time.OffsetDateTime
import java.time.ZoneOffset

trait Compiler[F[_], C, S, E, R, N] {
  def onRequest(cmd: CommandMessage[C])(
      run: RequestContext[C, S] => F[ProgramResult[S, E, R, N]]
  ): F[EitherNec[R, Unit]]
}

sealed trait ProgramResult[S, E, R, N]
object ProgramResult {
  final case class Accepted[S, E, R, N](
      newState: S,
      events: NonEmptyChain[E],
      notifications: Seq[N]
  ) extends ProgramResult[S, E, R, N]

  final case class Indecisive[S, E, R, N](
      notifications: Seq[N]
  ) extends ProgramResult[S, E, R, N]

  final case class Rejected[S, E, R, N](
      notifications: Seq[N],
      reasons: NonEmptyChain[R]
  ) extends ProgramResult[S, E, R, N]

  final case class Conflicted[S, E, R, N](
      reasons: NonEmptyChain[R]
  ) extends ProgramResult[S, E, R, N]

}
