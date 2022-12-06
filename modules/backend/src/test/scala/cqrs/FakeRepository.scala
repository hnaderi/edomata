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
package cqrs

import cats.data.*
import cats.effect.IO
import cats.effect.kernel.Ref
import cats.implicits.*
import edomata.backend.cqrs.FakeRepository.*
import edomata.core.*
import edomata.syntax.all.*
import munit.CatsEffectAssertions.*

final class FakeRepository[State, Event](
    state: AggregateState[State],
    _saved: Ref[IO, List[Interaction[State, Event]]]
) extends Repository[IO, State, Event] {

  override def notify(
      ctx: CommandMessage[?],
      notifications: NonEmptyChain[Event]
  ): IO[Unit] =
    _saved.update(_.prepended(Interaction.Notified(ctx, notifications)))

  override def get(id: StreamId): IO[AggregateS[State]] = state match {
    case a @ AggregateS(_, _) => IO(a)
    case _ => IO.raiseError(new Exception("don't know any state!"))
  }

  override def load(
      cmd: CommandMessage[?]
  ): IO[AggregateState[State]] = IO(state)

  override def save(
      ctx: CommandMessage[?],
      version: SeqNr,
      newState: State,
      events: Chain[Event]
  ): IO[Unit] = _saved.update(
    _.prepended(
      Interaction.Saved(
        ctx,
        version,
        newState,
        events
      )
    )
  )

  def saved: IO[List[Interaction[State, Event]]] = _saved.get
  def assert(item: Interaction[State, Event]): IO[Unit] =
    saved.assertEquals(List(item))
}

object FakeRepository {
  def apply[S, E](
      state: AggregateState[S]
  ): IO[FakeRepository[S, E]] =
    IO.ref(List.empty[Interaction[S, E]])
      .map(new FakeRepository(state, _))

  enum Interaction[S, E] {
    case Saved(
        ctx: CommandMessage[?],
        version: SeqNr,
        newState: S,
        events: Chain[E]
    )
    case Notified(
        ctx: CommandMessage[?],
        events: NonEmptyChain[E]
    )
  }
}
