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

import _root_.doobie.postgres.sqlstate
import cats.data.*
import cats.effect.kernel.Clock
import cats.effect.kernel.Concurrent
import cats.implicits.*
import doobie.*
import doobie.implicits.*
import doobie.util.transactor.Transactor
import edomata.backend.BackendError
import edomata.backend.CommandState.Redundant
import edomata.backend.SeqNr
import edomata.backend.StreamId
import edomata.backend.cqrs.*
import edomata.core.*

import java.time.OffsetDateTime
import java.time.ZoneOffset

private final class DoobieCQRSRepository[F[_]: Concurrent: Clock, S, N](
    trx: Transactor[F],
    states: Queries.State[S],
    o: Queries.Outbox[N],
    cmds: Queries.Commands,
    updates: NotificationsPublisher[F],
    handler: DoobieHandler[N]
)(using tc: StateModelTC[S])
    extends Repository[F, S, N] {

  private def _get(id: StreamId) = states.get(id).option.map {
    case None        => AggregateState(tc.initial, 0)
    case Some(value) => value
  }

  override def get(id: StreamId): F[AggregateState[S]] = _get(id).transact(trx)

  private val redundant: ConnectionIO[CommandState[S]] =
    Redundant.pure[ConnectionIO]
  private val currentTime: F[OffsetDateTime] =
    Clock[F].realTimeInstant.map(i => i.atOffset(ZoneOffset.UTC))

  override def load(cmd: CommandMessage[?]): F[CommandState[S]] = cmds
    .count(cmd.id)
    .unique
    .flatMap(c =>
      if c != 0 then redundant
      else _get(cmd.address).widen
    )
    .transact(trx)

  override def save(
      ctx: CommandMessage[?],
      version: SeqNr,
      newState: S,
      events: Chain[N]
  ): F[Unit] = for {
    now <- currentTime
    query = for {
      _ <- states
        .put(ctx.address, newState, version)
        .run
        .map(_ == 1)
        .ifM(FC.unit, FC.raiseError(BackendError.VersionConflict))

      _ <- NonEmptyList
        .fromFoldable(events.map((_, ctx.address, now, ctx.metadata)))
        .fold(FC.unit)(n => o.insertAll(n.toList).assertInserted(n.size))

      _ <- NonEmptyChain.fromChain(events).fold(FC.unit)(handler)

      _ <- cmds.insert(ctx).run.assertInserted

    } yield ()
    _ <- query
      .transact(trx)
      .attemptSomeSqlState { case sqlstate.class23.UNIQUE_VIOLATION =>
        BackendError.VersionConflict
      }
      .flatMap(_ => updates.notifyState >> updates.notifyOutbox)

  } yield ()

  override def notify(
      ctx: CommandMessage[?],
      notifications: NonEmptyChain[N]
  ): F[Unit] =
    currentTime
      .flatMap { now =>
        val ns = notifications.toList.map(
          (_, ctx.address, now, ctx.metadata)
        )
        o.insertAll(ns).assertInserted(ns.size).transact(trx)
      }
      .flatMap(_ => updates.notifyOutbox)

}
