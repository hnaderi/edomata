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
import cats.effect.IO
import cats.effect.kernel.Ref
import edomata.core.*

import FakeRepository.*

final class FakeRepository[S, E, R, N](
    actions: Ref[IO, List[Actions[S, E, R, N]]],
    state: CommandState[S, E, R],
    loaded: Ref[IO, List[CommandMessage[?]]]
) extends Repository[IO, S, E, R, N] {

  def load(cmd: CommandMessage[?]): IO[CommandState[S, E, R]] =
    loaded.update(_.prepended(cmd)).as(state)

  def append(
      ctx: RequestContext[?, ?],
      version: SeqNr,
      newState: S,
      events: NonEmptyChain[E],
      notifications: Chain[N]
  ): IO[Unit] = actions.update(
    _.prepended(
      Actions.Appended(
        ctx,
        version,
        newState,
        events,
        notifications
      )
    )
  )

  def notify(
      ctx: RequestContext[?, ?],
      notifications: NonEmptyChain[N]
  ): IO[Unit] = actions.update(
    _.prepended(Actions.Notified(ctx, notifications))
  )

  def listActions: IO[List[Actions[S, E, R, N]]] = actions.get
  def listLoaded: IO[List[CommandMessage[?]]] = loaded.get
}
object FakeRepository {
  def apply[S, E, R, N](
      state: CommandState[S, E, R]
  ): IO[FakeRepository[S, E, R, N]] = for {
    actions <- IO.ref(List.empty[Actions[S, E, R, N]])
    loaded <- IO.ref(List.empty[CommandMessage[?]])
  } yield new FakeRepository(actions, state, loaded)

  enum Actions[S, E, R, N] {
    case Appended(
        ctx: RequestContext[?, ?],
        version: SeqNr,
        newState: S,
        events: NonEmptyChain[E],
        notifications: Chain[N]
    )
    case Notified(
        ctx: RequestContext[?, ?],
        notifications: NonEmptyChain[N]
    )
  }
}
