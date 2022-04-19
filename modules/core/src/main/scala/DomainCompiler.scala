package edomata.core

import cats.Monad
import cats.data.Chain
import cats.data.EitherNec
import cats.data.NonEmptyChain
import cats.implicits.*

import java.time.OffsetDateTime
import java.time.ZoneOffset
import scala.util.NotGiven

type DomainService[F[_], C, R] = C => F[EitherNec[R, Unit]]

extension [F[_]: Monad, C, S, E, R, N](
    app: Edomaton[F, RequestContext[C, S], R, E, N, Unit]
)(using ModelTC[S, E, R]) {

  def execute(ctx: RequestContext[C, S]): F[ProgramResult[S, E, R, N]] =
    app.run(ctx).map { case Response(decision, notifs) =>
      ctx.state.perform(decision) match {
        case Decision.Accepted(evs, newState) =>
          ProgramResult.Accepted(newState, evs, notifs)
        case Decision.InDecisive(_) =>
          ProgramResult.Indecisive(notifs)
        case Decision.Rejected(errs) if decision.isRejected =>
          ProgramResult.Rejected(notifs, errs)
        case Decision.Rejected(errs) =>
          ProgramResult.Conflicted(errs)
      }
    }
}

extension [F[_]: Monad, C, S, E, R, N, T](
    app: Edomaton[F, RequestContext[C, S], R, E, N, T]
)(using NotGiven[T =:= Unit], ModelTC[S, E, R]) {

  def execute(ctx: RequestContext[C, S]): F[ProgramResult[S, E, R, N]] =
    app.void.execute(ctx)
}

enum ProgramResult[S, E, R, N] {
  case Accepted(
      newState: S,
      events: NonEmptyChain[E],
      notifications: Chain[N]
  )
  case Indecisive(
      notifications: Chain[N]
  )
  case Rejected(
      notifications: Chain[N],
      reasons: NonEmptyChain[R]
  )
  case Conflicted(
      reasons: NonEmptyChain[R]
  )
}
