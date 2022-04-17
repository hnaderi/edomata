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

private final class SkunkCompiler[F[_], E, N](
    pool: Resource[F, Session[F]],
    journal: Queries.Journal[E],
    outbox: Queries.Outbox[N],
    cmds: Queries.Commands
)(using F: Sync[F], clock: Clock[F])
    extends Compiler[F, E, N] {

  private val trx = pool.flatTap(_.transaction)
  private val currentTime =
    clock.realTimeInstant.map(_.atOffset(ZoneOffset.UTC))
  private val newId = F.delay(UUID.randomUUID)

  def append(
      ctx: RequestContext[?, ?],
      events: NonEmptyChain[E],
      notifications: Seq[N]
  ): F[Unit] = trx
    .use { s =>
      for {
        now <- currentTime
        evs <- events.toList.traverse(e =>
          newId.map(uid =>
            journal.InsertRow(
              uid,
              streamId = ctx.command.address,
              time = now,
              version = ???,
              e
            )
          )
        )
        _ <- s
          .prepare(journal.append(evs))
          .use(_.execute(evs))
          .assertInserted(evs.size)
        _ <- NonEmptyChain.fromSeq(notifications).fold(F.unit) { n =>
          val ns = notifications.toList.map((_, now))
          s.prepare(outbox.insertAll(ns))
            .use(_.execute(ns))
            .assertInserted(ns.size)
        }
        _ <- s.prepare(cmds.insert).use(_.execute(ctx.command)).assertInserted
      } yield ()
    }
    .adaptErr { case SqlState.UniqueViolation(ex) =>
      ???
    }

  def notify(
      ctx: RequestContext[?, ?],
      notifications: NonEmptyChain[N]
  ): F[Unit] = trx.use { s =>
    for {
      now <- currentTime
      ns = notifications.toList.map((_, now))
      _ <- s
        .prepare(outbox.insertAll(ns))
        .use(_.execute(ns))
        .assertInserted(ns.size)
    } yield ()
  }

  extension (self: F[Completion]) {
    def assertInserted(size: Int): F[Unit] = self.flatMap {
      case Completion.Insert(i) =>
        if i == size then F.unit else F.raiseError(???)
      case _ => F.raiseError(???)
    }
    def assertInserted: F[Unit] = assertInserted(1)
  }
}
