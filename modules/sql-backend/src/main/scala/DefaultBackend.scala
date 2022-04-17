package edomata.backend

import cats.data.EitherNec
import cats.data.NonEmptyChain
import cats.effect.Temporal
import cats.effect.implicits.*
import cats.implicits.*
import edomata.core.*
import fs2.Stream

import java.time.OffsetDateTime
import java.util.UUID
import scala.concurrent.duration.*

type SeqNr = Long
type EventVersion = Long
type StreamId = String

private val void: EitherNec[Nothing, Unit] = Right(())

final class DefaultBackend[F[_], S, E, R, N](
    compiler: Compiler[F, E, N],
    val outbox: OutboxReader[F, N],
    val journal: JournalReader[F, E],
    val repository: Repository[F, S, E, R],
    maxRetry: Int = 5,
    initialRetryDelay: FiniteDuration = 1.seconds
)(using
    F: Temporal[F],
    model: ModelTC[S, E, R]
) extends Backend[F, S, E, R, N] {
  private val voidF: F[EitherNec[R, Unit]] = void.pure[F]

  private def retry[T](max: Int, wait: FiniteDuration)(f: F[T]): F[T] =
    f.recoverWith {
      case BackendError.VersionConflict if max > 0 =>
        retry(max - 1, wait * 2)(f).delayBy(wait)
    }.adaptErr { case BackendError.VersionConflict =>
      BackendError.MaxRetryExceeded
    }

  def compile[C](
      app: Edomaton[F, RequestContext[C, S], R, E, N, Unit]
  ): DomainService[F, CommandMessage[C], R] = cmd =>
    retry(maxRetry, initialRetryDelay) {
      repository.get(cmd.address).flatMap {
        case AggregateState.Valid(s, rev) =>
          val ctx = cmd.buildContext(s)
          val res = app.execute(ctx)

          res.flatMap {
            case ProgramResult.Accepted(_, evs, notifs) =>
              compiler.append(ctx, rev, evs, notifs).as(void)
            case ProgramResult.Indecisive(notifs) =>
              NonEmptyChain
                .fromSeq(notifs)
                .fold(voidF)(compiler.notify(ctx, _).as(void))
            case ProgramResult.Rejected(notifs, errs) =>
              val res = errs.asLeft[Unit]
              NonEmptyChain
                .fromSeq(notifs)
                .fold(res.pure)(compiler.notify(ctx, _).as(res))
            case ProgramResult.Conflicted(errs) => errs.asLeft.pure
          }
        case AggregateState.Conflicted(ls, lEv, errs) => errs.asLeft.pure
      }
    }
}
