package edomata.core

import cats.Monad
import cats.data.EitherNec
import cats.data.NonEmptyChain
import cats.implicits.*

import java.time.OffsetDateTime
import java.time.ZoneOffset
import scala.util.NotGiven

import Domain.*

type DomainService[F[_], C, R] = C => F[EitherNec[R, Unit]]
object DomainService {
  def default[F[_]: Monad, C, S, E, R, N, M](
      cmdHandler: CommandHandler[F, C, S, E, R, N, M],
      app: Edomaton[F, RequestContext.Valid[C, S, M, R], R, E, N, Unit]
  ): DomainService[F, CommandMessage[C, M], R] = {
    val void: EitherNec[R, Unit] = Right(())
    val voidF = void.pure[F]

    def handle(cmd: CommandMessage[C, M]) =
      cmdHandler.onRequest(cmd) {
        case ctx @ RequestContext.Valid(_, state, _) =>
          app.run(ctx).flatMap { case Response(decision, notifs) =>
            state.perform(decision) match {
              case Decision.Accepted(evs, newState) =>
                cmdHandler.onAccept(ctx, evs, notifs).as(void)
              case Decision.InDecisive(_) =>
                cmdHandler.onIndecisive(ctx, notifs).as(void)
              case Decision.Rejected(errs) if decision.isRejected =>
                cmdHandler.onReject(ctx, notifs, errs).as(errs.asLeft)
              case Decision.Rejected(errs) =>
                cmdHandler.onConflict(ctx, errs).as(errs.asLeft)
            }
          }
        case ctx @ RequestContext.Conflict(errs) =>
          cmdHandler.onConflict(ctx, errs).as(errs.asLeft)
        case RequestContext.Redundant => voidF
      }

    handle(_)
  }
}

extension [F[_]: Monad, C, S, E, R, N, M](
    app: Edomaton[F, RequestContext.Valid[C, S, M, R], R, E, N, Unit]
) {
  def compile(
      cmdHandler: CommandHandler[F, C, S, E, R, N, M]
  ): DomainService[F, CommandMessage[C, M], R] =
    DomainService.default(cmdHandler, app)
}

extension [F[_]: Monad, C, S, E, R, N, M, T](
    app: Edomaton[F, RequestContext.Valid[C, S, M, R], R, E, N, T]
)(using NotGiven[T =:= Unit]) {
  def compile(
      cmdHandler: CommandHandler[F, C, S, E, R, N, M]
  ): DomainService[F, CommandMessage[C, M], R] =
    DomainService.default(cmdHandler, app.void)
}
