package edomata.backend

import cats.Monad
import cats.data.EitherNec
import cats.data.NonEmptyChain
import cats.effect.Temporal
import cats.effect.implicits.*
import cats.implicits.*
import edomata.core.*

import scala.concurrent.duration.FiniteDuration

object CommandHandler {
  private val void: EitherNec[Nothing, Unit] = Right(())

  def apply[F[_]: Monad, C, S, E, R, N](
      repository: Repository[F, S, E, R, N],
      app: Edomaton[F, RequestContext[C, S], R, E, N, Unit]
  )(using ModelTC[S, E, R]): DomainService[F, CommandMessage[C], R] =
    val voidF: F[EitherNec[R, Unit]] = void.pure[F]
    cmd =>
      repository.load(cmd).flatMap {
        case AggregateState.Valid(s, rev) =>
          val ctx = cmd.buildContext(s)
          val res = app.execute(ctx)

          res.flatMap {
            case ProgramResult.Accepted(ns, evs, notifs) =>
              repository.append(ctx, rev, ns, evs, notifs).as(void)
            case ProgramResult.Indecisive(notifs) =>
              NonEmptyChain
                .fromSeq(notifs)
                .fold(voidF)(repository.notify(ctx, _).as(void))
            case ProgramResult.Rejected(notifs, errs) =>
              val res = errs.asLeft[Unit]
              NonEmptyChain
                .fromSeq(notifs)
                .fold(res.pure)(repository.notify(ctx, _).as(res))
            case ProgramResult.Conflicted(errs) => errs.asLeft.pure
          }
        case AggregateState.Conflicted(ls, lEv, errs) => errs.asLeft.pure
        case CommandState.Redundant                   => voidF
      }

  private[backend] def retry[F[_]: Temporal, T](max: Int, wait: FiniteDuration)(
      f: F[T]
  ): F[T] =
    f.recoverWith {
      case BackendError.VersionConflict if max > 0 =>
        retry(max - 1, wait * 2)(f).delayBy(wait)
    }.adaptErr { case BackendError.VersionConflict =>
      BackendError.MaxRetryExceeded
    }
}
