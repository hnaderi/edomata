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

enum ProgramResult[S, E, R, N] {
  case Accepted(
      newState: S,
      events: NonEmptyChain[E],
      notifications: Seq[N]
  )
  case Indecisive(
      notifications: Seq[N]
  )
  case Rejected(
      notifications: Seq[N],
      reasons: NonEmptyChain[R]
  )
  case Conflicted(
      reasons: NonEmptyChain[R]
  )
}
