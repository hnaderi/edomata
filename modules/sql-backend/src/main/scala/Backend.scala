package edomata.backend

import cats.data.NonEmptyChain
import edomata.core.*
import fs2.Stream
import cats.implicits.*
import cats.data.EitherNec

import java.time.OffsetDateTime
import java.util.UUID
import cats.Monad

type SeqNr = Long
type EventVersion = Long
type StreamId = String

private val void: EitherNec[Nothing, Unit] = Right(())

abstract class Backend[F[_], S, E, R, N](compiler: Compiler[F, E, N])(using
    F: Monad[F],
    model: ModelTC[S, E, R]
) {
  private val voidF: F[EitherNec[R, Unit]] = void.pure[F]

  def compile[C](
      app: Edomaton[F, RequestContext[C, S], R, E, N, Unit]
  ): DomainService[F, CommandMessage[C], R] = cmd =>
    repository.get(cmd.address).flatMap {
      case AggregateState.Valid(s, rev) =>
        val ctx = cmd.buildContext(s)
        val res = app.execute(ctx)

        res.flatMap {
          case ProgramResult.Accepted(_, evs, notifs) =>
            compiler.append(ctx, evs, notifs).as(void)
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

  lazy val outbox: OutboxReader[F, N]
  lazy val journal: JournalReader[F, E]
  lazy val repository: Repository[F, S, E, R]
}

trait Compiler[F[_], E, N] {
  def append(
      ctx: RequestContext[?, ?],
      events: NonEmptyChain[E],
      notifications: Seq[N]
  ): F[Unit]

  def notify(
      ctx: RequestContext[?, ?],
      notifications: NonEmptyChain[N]
  ): F[Unit]
}
