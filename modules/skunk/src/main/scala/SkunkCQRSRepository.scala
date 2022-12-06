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
import cats.data.*
import cats.effect.Concurrent
import cats.effect.implicits.*
import cats.effect.kernel.Clock
import cats.effect.kernel.Resource
import cats.implicits.*
import edomata.backend.BackendError
import edomata.backend.CommandState
import edomata.backend.SeqNr
import edomata.backend.StreamId
import edomata.backend.cqrs.*
import edomata.core.*
import fs2.Stream
import skunk.*
import skunk.data.Completion
import skunk.implicits.*

import java.time.OffsetDateTime
import java.time.ZoneOffset
import scala.concurrent.duration.*

private final class SkunkCQRSRepository[F[_]: Clock, S, N](
    pool: Resource[F, Session[F]],
    state: Queries.State[S],
    outbox: Queries.Outbox[N],
    cmds: Queries.Commands,
    updates: NotificationsPublisher[F]
)(using tc: StateModelTC[S], F: Concurrent[F])
    extends Repository[F, S, N] {
  private val redundant: F[AggregateState[S]] =
    CommandState.Redundant.pure[F]
  private val trx = pool.flatTap(_.transaction)

  private def _get(s: Session[F], id: StreamId): F[AggregateS[S]] =
    s.prepare(state.get).use(_.option(id)).map {
      case None        => AggregateS(tc.initial, 0)
      case Some(value) => value
    }

  override def get(id: StreamId): F[AggregateS[S]] =
    pool.use(_get(_, id))

  override def load(cmd: CommandMessage[?]): F[AggregateState[S]] =
    pool.use(s =>
      s.prepare(cmds.count)
        .use(_.unique(cmd.id))
        .flatMap(c =>
          if c != 0 then redundant
          else _get(s, cmd.address).widen
        )
    )

  override def save(
      ctx: CommandMessage[?],
      version: SeqNr,
      newState: S,
      events: Chain[N]
  ): F[Unit] = trx
    .use { s =>
      for {
        now <- currentTime[F]
        _ <- s
          .prepare(state.put)
          .use(_.execute(ctx.address ~ newState ~ version))
          .flatMap {
            case Completion.Insert(1) | Completion.Update(1) => F.unit
            case Completion.Insert(0) | Completion.Update(0) =>
              F.raiseError(BackendError.VersionConflict)
            case other =>
              F.raiseError(
                BackendError.PersistenceError(
                  s"expected to upsert state, but got invalid response from database! response: $other"
                )
              )
          }
        _ <- NonEmptyChain.fromChain(events).fold(F.unit) { n =>
          val ns = events.toList
            .map((_, ctx.address, now, ctx.metadata))
          s.prepare(outbox.insertAll(ns))
            .use(_.execute(ns))
            .assertInserted(ns.size)
        }
        _ <- s
          .prepare(cmds.insert)
          .use(_.execute(ctx))
          .assertInserted
      } yield ()
    }
    .adaptErr { case SqlState.UniqueViolation(ex) =>
      BackendError.VersionConflict
    }
    .flatMap(_ => updates.notifyState >> updates.notifyOutbox)

  override def notify(
      ctx: CommandMessage[?],
      notifications: NonEmptyChain[N]
  ): F[Unit] = trx
    .use { s =>
      for {
        now <- currentTime
        ns = notifications.toList.map(
          (_, ctx.address, now, ctx.metadata)
        )
        _ <- s
          .prepare(outbox.insertAll(ns))
          .use(_.execute(ns))
          .assertInserted(ns.size)
      } yield ()
    }
    .flatMap(_ => updates.notifyOutbox)

}
