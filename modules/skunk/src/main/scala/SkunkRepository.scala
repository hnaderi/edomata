/*
 * Copyright 2021 Hossein Naderi
 *
 * Licensed under the Apache License, Version 2.0 (the "License");
 * you may not use this file except in compliance with the License.
 * You may obtain a copy of the License at
 *
 *     http://www.apache.org/licenses/LICENSE-2.0
 *
 * Unless required by applicable law or agreed to in writing, software
 * distributed under the License is distributed on an "AS IS" BASIS,
 * WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
 * See the License for the specific language governing permissions and
 * limitations under the License.
 */

package edomata.skunk

import _root_.skunk.*
import cats.data.Chain
import cats.data.NonEmptyChain
import cats.effect.kernel.Resource
import cats.effect.kernel.Sync
import cats.effect.std.UUIDGen
import cats.implicits.*
import edomata.backend.*
import edomata.core.*

private final class SkunkRepository[F[_], S, E, R, N](
    pool: Resource[F, Session[F]],
    journal: Queries.Journal[E],
    outbox: Queries.Outbox[N],
    cmds: Queries.Commands,
    repository: RepositoryReader[F, S, E, R],
    updates: NotificationsPublisher[F]
)(using F: Sync[F])
    extends Repository[F, S, E, R, N] {

  private val trx = pool.flatTap(_.transaction)
  private val newId = UUIDGen[F].randomUUID
  private val redundant: F[CommandState[S, E, R]] =
    CommandState.Redundant.pure[F]

  def load(cmd: CommandMessage[?]): F[CommandState[S, E, R]] =
    pool
      .use(_.prepare(cmds.count).flatMap(_.unique(cmd.id)))
      .flatMap(c =>
        if c != 0 then redundant
        else repository.get(cmd.address).widen
      )

  def append(
      ctx: RequestContext[?, ?],
      version: SeqNr,
      newState: S,
      events: NonEmptyChain[E],
      notifications: Chain[N]
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
          .flatMap(_.execute(evs))
          .assertInserted(evs.size)
        _ <- NonEmptyChain.fromChain(notifications).fold(F.unit) { n =>
          val ns = notifications.toList
            .map((_, ctx.command.address, now, ctx.command.metadata))
          s.prepare(outbox.insertAll(ns))
            .flatMap(_.execute(ns))
            .assertInserted(ns.size)
        }
        _ <- s
          .prepare(cmds.insert)
          .flatMap(_.execute(ctx.command))
          .assertInserted
      } yield ()
    }
    .adaptErr { case SqlState.UniqueViolation(ex) =>
      BackendError.VersionConflict
    }
    .flatMap(_ => updates.notifyJournal >> updates.notifyOutbox)

  def notify(
      ctx: RequestContext[?, ?],
      notifications: NonEmptyChain[N]
  ): F[Unit] = trx
    .use { s =>
      for {
        now <- currentTime
        ns = notifications.toList.map(
          (_, ctx.command.address, now, ctx.command.metadata)
        )
        _ <- s
          .prepare(outbox.insertAll(ns))
          .flatMap(_.execute(ns))
          .assertInserted(ns.size)
      } yield ()
    }
    .flatMap(_ => updates.notifyOutbox)
}
