package edomata.backend.rev2

import cats.Monad
import cats.data.EitherNec
import cats.data.NonEmptyChain
import cats.effect.Clock
import cats.effect.Concurrent
import cats.effect.kernel.Resource
import cats.implicits.*
import edomata.core.CommandHandler
import edomata.core.CommandMessage
import edomata.core.Model
import edomata.core.RequestContext2
import fs2.Chunk
import fs2.Pipe
import fs2.Stream

import java.time.OffsetDateTime
import java.time.ZoneOffset

object SQLCommandHandler {
  def default[F[_]: Concurrent: Clock, C, S, E, R, N, M](
      persistence: ESPersistence[F, Model.Of[S, E, R], E, R, N, M]
  ): CommandHandler[F, C, S, E, R, N, M] = new CommandHandler {
    private val currentTime =
      Clock[F].realTimeInstant.map(_.atOffset(ZoneOffset.UTC))
    private def publish(notifs: Seq[N]) =
      NonEmptyChain
        .fromSeq(notifs)
        .map(persistence.outbox)
        .getOrElse(Monad[F].unit)

    def onRequest[T](
        cmd: CommandMessage[C, M]
    )(f: RequestContext2[C, Model.Of[S, E, R], M, R] => F[T]): F[T] =
      persistence.transaction.use(_ =>
        persistence
          .containsCmd(cmd.id)
          .ifM(
            persistence
              .readFromJournal(cmd.id)
              .map {
                case AggregateState.Valid(v, s) =>
                  cmd.buildContext(s, v)
                case AggregateState.Failed(lastState, event, errs) =>
                  RequestContext2.Conflict(errs)
              },
            RequestContext2.Redundant.pure[F]
          )
          .flatMap(f)
      )

    def onAccept(
        ctx: RequestContext2.Valid[C, Model.Of[S, E, R], M, R],
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
        ctx: RequestContext2.Valid[C, Model.Of[S, E, R], M, R],
        notifications: Seq[N]
    ): F[Unit] =
      publish(notifications) >> persistence.appendCmdLog(ctx.command)

    def onReject(
        ctx: RequestContext2.Valid[C, Model.Of[S, E, R], M, R],
        notifications: Seq[N],
        reasons: NonEmptyChain[R]
    ): F[Unit] = publish(notifications)

    def onConflict(
        ctx: RequestContext2[C, Model.Of[S, E, R], M, R],
        reasons: NonEmptyChain[R]
    ): F[Unit] = Monad[F].unit
  }

  private[backend] final class Partial[D](private val dummy: Boolean = true)
      extends AnyVal {
    import edomata.core.Domain.*
    def build[F[_]: Concurrent: Clock](
        persistence: ESPersistence[
          F,
          Model.Of[StateFor[D], EventFor[D], RejectionFor[D]],
          EventFor[D],
          RejectionFor[D],
          NotificationFor[D],
          MetadataFor[D]
        ]
    ): CommandHandler[F, CommandFor[D], StateFor[D], EventFor[D], RejectionFor[
      D
    ], NotificationFor[D], MetadataFor[D]] = default(persistence)
  }

  def in[Domain]: Partial[Domain] = Partial()
}
