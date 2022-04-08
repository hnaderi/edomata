package edomata.backend.rev2

import cats.Monad
import cats.data.EitherNec
import cats.data.NonEmptyChain
import cats.effect.Concurrent
import cats.effect.kernel.Clock
import cats.implicits.*
import edomata.core.Decision
import edomata.core.Domain.*
import edomata.core.Response
import edomata.core.*

import java.time.OffsetDateTime
import java.time.ZoneOffset

type DomainService[F[_], C, R] = C => F[EitherNec[R, Unit]]
object DomainService {
  def default[F[_]: Concurrent: Clock, C, S, E, R, N, M](
      cmdHandler: CommandHandler[F, C, S, E, R, N, M],
      app: ServiceMonad[F, RequestContext2.Valid[C, S, M, R], R, E, N, Unit]
  ): DomainService[F, CommandMessage[C, M], R] = {
    val void: EitherNec[R, Unit] = Right(())
    val voidF = void.pure[F]

    def handle(cmd: CommandMessage[C, M]) =
      cmdHandler.onRequest(cmd).use {
        case ctx @ RequestContext2.Valid(_, state, _) =>
          app.run(ctx).flatMap { case ResponseMonad(decision, notifs) =>
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
        case ctx @ RequestContext2.Conflict(errs) =>
          cmdHandler.onConflict(ctx, errs).as(errs.asLeft)
        case RequestContext2.Redundant => voidF
      }

    handle(_)
  }

  extension [F[_]: Concurrent: Clock, C, S, E, R, N, M](
      app: ServiceMonad[F, RequestContext2[C, S, M, R], R, E, N, Unit]
  ) {
    def compile(
        cmdHandler: CommandHandler[F, C, S, E, R, N, M]
    ): DomainService[F, CommandMessage[C, M], R] = default(cmdHandler, app)
  }
}
