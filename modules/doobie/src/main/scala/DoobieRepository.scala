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

package edomata.doobie

import _root_.doobie.*
import _root_.doobie.implicits.*
import _root_.doobie.postgres.sqlstate
import cats.data.Chain
import cats.data.NonEmptyChain
import cats.effect.kernel.Clock
import cats.effect.kernel.Sync
import cats.implicits.*
import edomata.backend.*
import edomata.core.*

import java.time.OffsetDateTime
import java.time.ZoneOffset
import cats.effect.std.UUIDGen

private final class DoobieRepository[F[_], S, E, R, N](
    trx: Transactor[F],
    j: Queries.Journal[E],
    o: Queries.Outbox[N],
    cmds: Queries.Commands,
    repository: RepositoryReader[F, S, E, R],
    updates: NotificationsPublisher[F]
)(using
    F: Sync[F],
    clock: Clock[F]
) extends Repository[F, S, E, R, N] {

  private val newId = UUIDGen[F].randomUUID
  private val redundant: F[CommandState[S, E, R]] =
    CommandState.Redundant.pure[F]
  private val currentTime: F[OffsetDateTime] =
    clock.realTimeInstant.map(i => i.atOffset(ZoneOffset.UTC))

  def load(cmd: CommandMessage[?]): F[CommandState[S, E, R]] =
    cmds
      .count(cmd.id)
      .unique
      .transact(trx)
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
  ): F[Unit] = for {
    now <- currentTime
    evs <- events.toList.zipWithIndex.traverse((e, i) =>
      newId.map(uid =>
        j.InsertRow(
          uid,
          streamId = ctx.command.address,
          time = now,
          version = version + i,
          e
        )
      )
    )
    query = for {
      _ <- j.append(evs).assertInserted(events.size)
      _ <- NonEmptyChain.fromChain(notifications).fold(FC.unit) { n =>
        o.insertAll(
          n.toList.map((_, ctx.command.address, now, ctx.command.metadata))
        ).assertInserted(notifications.size)
      }
      _ <- cmds.insert(ctx.command).run.assertInserted
    } yield ()

    _ <- query.transact(trx).attemptSomeSqlState {
      case sqlstate.class23.UNIQUE_VIOLATION => BackendError.VersionConflict
    }
    _ <- updates.notifyJournal
    _ <- updates.notifyOutbox
  } yield ()

  def notify(
      ctx: RequestContext[?, ?],
      notifications: NonEmptyChain[N]
  ): F[Unit] = for {
    now <- currentTime
    ns = notifications.toList.map(
      (_, ctx.command.address, now, ctx.command.metadata)
    )
    _ <- o.insertAll(ns).assertInserted(notifications.size).transact(trx)
    _ <- updates.notifyOutbox
  } yield ()
}
