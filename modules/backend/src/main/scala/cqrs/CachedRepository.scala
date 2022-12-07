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

package edomata.backend.cqrs

import cats.Monad
import cats.data.*
import cats.effect.kernel.Async
import cats.implicits.*
import edomata.backend.Cache
import edomata.backend.CommandState.Redundant
import edomata.backend.CommandStore
import edomata.backend.SeqNr
import edomata.backend.StreamId
import edomata.core.*

private final class CachedRepository[F[_]: Monad, S, E] private (
    commands: CommandStore[F],
    cache: Cache[F, StreamId, AggregateState[S]],
    underlying: Repository[F, S, E]
) extends Repository[F, S, E] {

  override def get(id: StreamId): F[AggregateState[S]] = underlying.get(id)

  override def load(cmd: CommandMessage[?]): F[CommandState[S]] = commands
    .contains(cmd.id)
    .ifM(
      Redundant.pure,
      cache.get(cmd.address).flatMap {
        case None        => underlying.load(cmd)
        case Some(value) => value.pure
      }
    )

  override def save(
      ctx: CommandMessage[?],
      version: SeqNr,
      newState: S,
      events: Chain[E]
  ): F[Unit] = underlying.save(ctx, version, newState, events) >>
    cache.replace(ctx.address, AggregateState(newState, version + 1))(
      _.version <= version
    ) >>
    commands.append(ctx)

  override def notify(
      ctx: CommandMessage[?],
      notifications: NonEmptyChain[E]
  ): F[Unit] = underlying.notify(ctx, notifications)

}

private object CachedRepository {
  def build[F[_]: Async, S, E](
      underlying: Repository[F, S, E],
      size: Int = 1000,
      maxCommands: Int = 1000
  ): F[CachedRepository[F, S, E]] = for {
    cache <- Cache.lru[F, StreamId, AggregateState[S]](size)
    cmds <- CommandStore.inMem(maxCommands)
  } yield from(underlying, cmds, cache)

  def from[F[_]: Async, S, E](
      underlying: Repository[F, S, E],
      commands: CommandStore[F],
      cache: Cache[F, StreamId, AggregateState[S]]
  ): CachedRepository[F, S, E] =
    new CachedRepository(commands, cache, underlying)

  def apply[F[_]: Async, S, E](
      underlying: Repository[F, S, E],
      commands: CommandStore[F],
      size: Int = 1000
  ): F[CachedRepository[F, S, E]] =
    Cache
      .lru[F, StreamId, AggregateState[S]](size)
      .map(new CachedRepository(commands, _, underlying))
}
