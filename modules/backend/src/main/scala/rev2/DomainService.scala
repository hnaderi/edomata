package edomata.backend

import cats.Monad
import cats.data.EitherNec
import cats.data.NonEmptyChain
import cats.effect.kernel.Clock
import cats.implicits.*
import edomata.core.Decision
import edomata.core.Domain.*
import edomata.core.Response
import edomata.core.*
import edomata.eventsourcing.*

import java.time.OffsetDateTime
import java.time.ZoneOffset

type DomainService[F[_], C, R] = C => F[EitherNec[R, Unit]]
object DomainService {
  def default[F[_]: Monad: Clock, C, S, E, R, N, M](
      persistence: ESPersistence[F, S & Model[S, E, R], E, R, N, M],
      app: ServiceMonad[F, RequestContext2[C, S, M], R, E, N, Unit]
  ): DomainService[F, CommandMessage[C, M], R] = {
    val F = Monad[F]
    val void: EitherNec[R, Unit] = Right(())
    val voidF = void.pure[F]

    val currentTime = Clock[F].realTimeInstant.map(_.atOffset(ZoneOffset.UTC))
    def handleDecision(
        cmd: CommandMessage[C, M],
        ctx: RequestContext2[C, S & Model[S, E, R], M],
        version: Long,
        decision: Decision[R, E, Unit],
        notifs: Seq[N]
    ): F[EitherNec[R, Unit]] =
      currentTime.flatMap { now =>
        val publish =
          if (notifs.isEmpty) then F.unit else persistence.outbox(notifs)

        ctx.state.perform(decision) match {
          case Decision.Accepted(evs, newState) =>
            persistence.appendJournal(ctx.id, now, version, evs) >>
              persistence.appendCmdLog(cmd) >>
              publish.as(void)
          case Decision.InDecisive(_) =>
            publish.as(void)
          case Decision.Rejected(err) if decision.isRejected =>
            publish.as(Left(err))
          case Decision.Rejected(err) => ???
        }
      }

    def handle(cmd: CommandMessage[C, M]) =
      persistence.readFromJournal(cmd.address).flatMap {
        case Right(AggregateState(version, state)) =>
          val ctx = cmd.buildContext(state)
          app.run(ctx).flatMap { case ResponseMonad(decision, notifs) =>
            handleDecision(cmd, ctx, version, decision, notifs)
          }
        case e @ Left(err) => Left(err).pure[F]
      }

    cmd =>
      persistence.transaction(
        persistence.containsCmd(cmd.id).ifM(voidF, handle(cmd))
      )
  }

  extension [F[_]: Monad: Clock, C, S, E, R, N, M](
      app: ServiceMonad[F, RequestContext2[C, S, M], R, E, N, Unit]
  ) {
    def compile(
        persistence: ESPersistence[F, S & Model[S, E, R], E, R, N, M]
    ): DomainService[F, CommandMessage[C, M], R] = default(persistence, app)
  }
}
