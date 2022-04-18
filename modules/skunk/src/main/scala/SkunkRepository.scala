package edomata.backend

import cats.data.NonEmptyChain
import cats.effect.Concurrent
import cats.effect.kernel.Clock
import cats.effect.kernel.Resource
import cats.effect.kernel.Sync
import cats.implicits.*
import edomata.core.*
import skunk.*
import skunk.data.Completion

import java.time.ZoneOffset
import java.util.UUID

private final class SkunkRepository[F[_], S, E, R, N](
    pool: Resource[F, Session[F]],
    journal: Queries.Journal[E],
    outbox: Queries.Outbox[N],
    cmds: Queries.Commands,
    commands: CommandStore[F],
    snapshot: SnapshotStore[F, S, E, R],
    repository: RepositoryReader[F, S, E, R]
)(using F: Sync[F], clock: Clock[F])
    extends Repository[F, S, E, R, N] {

  private val trx = pool.flatTap(_.transaction)
  private val newId = F.delay(UUID.randomUUID)

  def load(cmd: CommandMessage[?]): F[CommandState[S, E, R]] =
    commands.contains(cmd.id).flatMap { redundant =>
      if redundant then CommandState.Redundant.pure
      else
        snapshot.get(cmd.address).flatMap {
          case Some(s) => s.pure
          case None    => repository.get(cmd.address).widen
        }
    }

  def append(
      ctx: RequestContext[?, ?],
      version: SeqNr,
      newState: S,
      events: NonEmptyChain[E],
      notifications: Seq[N]
  ): F[Unit] = trx
    .use { s =>
      for {
        now <- currentTime[F]
        evs <- events.toList.zipWithIndex.traverse((e, i) =>
          newId.map(uid =>
            journal.InsertRow(
              uid,
              streamId = ctx.command.address,
              time = now,
              version = version + i,
              e
            )
          )
        )
        _ <- s
          .prepare(journal.append(evs))
          .use(_.execute(evs))
          .assertInserted(evs.size)
        _ <- NonEmptyChain.fromSeq(notifications).fold(F.unit) { n =>
          val ns = notifications.toList.map((_, now, ctx.command.metadata))
          s.prepare(outbox.insertAll(ns))
            .use(_.execute(ns))
            .assertInserted(ns.size)
        }
        _ <- s
          .prepare(cmds.insert)
          .use(_.execute(ctx.command))
          .assertInserted
      } yield ()
    }
    .adaptErr { case SqlState.UniqueViolation(ex) =>
      BackendError.VersionConflict
    }

  def notify(
      ctx: RequestContext[?, ?],
      notifications: NonEmptyChain[N]
  ): F[Unit] = trx.use { s =>
    for {
      now <- currentTime
      ns = notifications.toList.map((_, now, ctx.command.metadata))
      _ <- s
        .prepare(outbox.insertAll(ns))
        .use(_.execute(ns))
        .assertInserted(ns.size)
    } yield ()
  }

  extension (self: F[Completion]) {
    def assertInserted(size: Int): F[Unit] = self.flatMap {
      case Completion.Insert(i) =>
        if i == size then F.unit
        else
          F.raiseError(
            BackendError.PersistenceError(
              s"expected to insert exactly $size, but inserted $i"
            )
          )
      case other =>
        F.raiseError(
          BackendError.PersistenceError(
            s"expected to receive insert response, but received: $other"
          )
        )
    }
    def assertInserted: F[Unit] = assertInserted(1)
  }
}
