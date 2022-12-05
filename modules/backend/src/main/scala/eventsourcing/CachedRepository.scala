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

package edomata.backend
package eventsourcing

import cats.data.Chain
import cats.data.NonEmptyChain
import cats.effect.Concurrent
import cats.implicits.*
import edomata.core.*

final class CachedRepository[F[_]: Concurrent, S, E, R, N](
    underlying: Repository[F, S, E, R, N],
    cmds: CommandStore[F],
    snapshot: SnapshotStore[F, S]
) extends Repository[F, S, E, R, N] {

  private val redundant: F[CommandState[S, E, R]] =
    CommandState.Redundant.pure[F]

  def load(cmd: CommandMessage[?]): F[CommandState[S, E, R]] = cmds
    .contains(cmd.id)
    .ifM(
      redundant,
      snapshot.getFast(cmd.address).flatMap {
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
