package edomata.core

import cats.Monad
import cats.data.EitherNec
import cats.data.NonEmptyChain
import cats.implicits.*

import java.time.OffsetDateTime
import java.time.ZoneOffset
import scala.util.NotGiven

type DomainService[F[_], C, R] = C => F[EitherNec[R, Unit]]

object DomainCompiler {
  def default[F[_]: Monad, C, S, E, R, N](
      compiler: Compiler[F, C, S, E, R, N],
      app: Edomaton[F, RequestContext[C, S], R, E, N, Unit]
  )(using ModelTC[S, E, R]): DomainService[F, CommandMessage[C], R] = {

    def handle(cmd: CommandMessage[C]) =
      compiler.onRequest(cmd) { ctx =>
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

    handle(_)
  }
}

extension [F[_]: Monad, C, S, E, R, N](
    app: Edomaton[F, RequestContext[C, S], R, E, N, Unit]
) {
  def compile(
      compiler: Compiler[F, C, S, E, R, N]
  )(using ModelTC[S, E, R]): DomainService[F, CommandMessage[C], R] =
    DomainCompiler.default(compiler, app)
}

extension [F[_]: Monad, C, S, E, R, N, T](
    app: Edomaton[F, RequestContext[C, S], R, E, N, T]
)(using NotGiven[T =:= Unit], ModelTC[S, E, R]) {
  def compile(
      compiler: Compiler[F, C, S, E, R, N]
  ): DomainService[F, CommandMessage[C], R] =
    DomainCompiler.default(compiler, app.void)
}