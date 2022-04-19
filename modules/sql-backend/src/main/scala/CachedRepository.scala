package edomata.backend

import cats.data.Chain
import cats.data.NonEmptyChain
import cats.effect.Concurrent
import cats.implicits.*
import edomata.core.*

final class CachedRepository[F[_]: Concurrent, S, E, R, N](
    underlying: Repository[F, S, E, R, N],
    cmds: CommandStore[F],
    snapshot: SnapshotStore[F, S, E, R]
) extends Repository[F, S, E, R, N] {
  def load(cmd: CommandMessage[?]): F[CommandState[S, E, R]] = cmds
    .contains(cmd.id)
    .ifM(
      CommandState.Redundant.pure,
      snapshot.get(cmd.address).flatMap {
        case Some(s) => s.pure
        case None    => underlying.load(cmd)
      }
    )

  def append(
      ctx: RequestContext[?, ?],
      version: SeqNr,
      newState: S,
      events: NonEmptyChain[E],
      notifications: Chain[N]
  ): F[Unit] =
    underlying.append(ctx, version, newState, events, notifications) >>
      cmds.append(ctx.command) >>
      snapshot.put(
        ctx.command.address,
        AggregateState.Valid(newState, version + events.size)
      )

  def notify(
      ctx: RequestContext[?, ?],
      notifications: NonEmptyChain[N]
  ): F[Unit] = underlying.notify(ctx, notifications)
}
