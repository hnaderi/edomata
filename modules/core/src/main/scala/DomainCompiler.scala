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

object DomainCompiler {
  def default[F[_]: Monad, C, S, E, R, N, M](
      compiler: Compiler[F, C, S, E, R, N, M],
      app: Edomaton[F, RequestContext[C, S, M], R, E, N, Unit]
  ): DomainService[F, CommandMessage[C, M], R] = {

    def handle(cmd: CommandMessage[C, M]) =
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

extension [F[_]: Monad, C, S, E, R, N, M](
    app: Edomaton[F, RequestContext[C, S, M], R, E, N, Unit]
) {
  def compile(
      compiler: Compiler[F, C, S, E, R, N, M]
  ): DomainService[F, CommandMessage[C, M], R] =
    DomainCompiler.default(compiler, app)
}

extension [F[_]: Monad, C, S, E, R, N, M, T](
    app: Edomaton[F, RequestContext[C, S, M], R, E, N, T]
)(using NotGiven[T =:= Unit]) {
  def compile(
      compiler: Compiler[F, C, S, E, R, N, M]
  ): DomainService[F, CommandMessage[C, M], R] =
    DomainCompiler.default(compiler, app.void)
}
