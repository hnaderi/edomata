package edomata.backend.rev2

import cats.Monad
import cats.data.NonEmptyChain
import cats.data.EitherNec
import cats.effect.kernel.Clock
import cats.implicits.*
import edomata.core.CommandMessage
import edomata.core.*

import java.time.OffsetDateTime
import java.time.ZoneOffset
import cats.effect.kernel.Resource

trait CommandHandler[F[_], C, S, E, R, N, M] {
  def onRequest(
      cmd: CommandMessage[C, M]
  ): Resource[F, RequestContext2[C, S & Model[S, E, R], M, R]]

  def onAccept(
      ctx: RequestContext2.Valid[C, S & Model[S, E, R], M, R],
      events: NonEmptyChain[E],
      notifications: Seq[N]
  ): F[Unit]

  def onIndecisive(
      ctx: RequestContext2.Valid[C, S & Model[S, E, R], M, R],
      notifications: Seq[N]
  ): F[Unit]

  def onReject(
      ctx: RequestContext2.Valid[C, S & Model[S, E, R], M, R],
      notifications: Seq[N],
      reasons: NonEmptyChain[R]
  ): F[Unit]

  def onConflict(
      ctx: RequestContext2[C, S & Model[S, E, R], M, R],
      reasons: NonEmptyChain[R]
  ): F[Unit]
}

object CommandHandler {
  def default[F[_]: Monad: Clock, C, S, E, R, N, M](
      persistence: ESPersistence[F, S & Model[S, E, R], E, R, N, M]
  ): CommandHandler[F, C, S, E, R, N, M] = new CommandHandler {
    private val currentTime =
      Clock[F].realTimeInstant.map(_.atOffset(ZoneOffset.UTC))
    private def publish(notifs: Seq[N]) =
      NonEmptyChain
        .fromSeq(notifs)
        .map(persistence.outbox)
        .getOrElse(Monad[F].unit)

    def onRequest(
        cmd: CommandMessage[C, M]
    ): Resource[F, RequestContext2[C, S & Model[S, E, R], M, R]] =
      persistence.transaction.evalMap(_ =>
        persistence
          .containsCmd(cmd.id)
          .ifM(
            persistence
              .readFromJournal(cmd.id)
              .map {
                case Right(AggregateState(v, s)) =>
                  cmd.buildContext(s, v)
                case Left(errs) =>
                  RequestContext2.Conflict(errs)
              },
            RequestContext2.Redundant.pure[F]
          )
      )

    def onAccept(
        ctx: RequestContext2.Valid[C, S & Model[S, E, R], M, R],
        events: NonEmptyChain[E],
        notifications: Seq[N]
    ): F[Unit] =
      currentTime.flatMap(now =>
        persistence.appendJournal(
          ctx.command.address,
          now,
          ctx.version,
          events
        ) >>
          persistence.appendCmdLog(ctx.command) >>
          publish(notifications)
      )

    def onIndecisive(
        ctx: RequestContext2.Valid[C, S & Model[S, E, R], M, R],
        notifications: Seq[N]
    ): F[Unit] =
      publish(notifications) >> persistence.appendCmdLog(ctx.command)

    def onReject(
        ctx: RequestContext2.Valid[C, S & Model[S, E, R], M, R],
        notifications: Seq[N],
        reasons: NonEmptyChain[R]
    ): F[Unit] = publish(notifications)

    def onConflict(
        ctx: RequestContext2[C, S & Model[S, E, R], M, R],
        reasons: NonEmptyChain[R]
    ): F[Unit] = ???
  }
}
