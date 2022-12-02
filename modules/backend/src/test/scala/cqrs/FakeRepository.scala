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

import cats.data.Chain
import cats.effect.IO
import cats.effect.kernel.Ref
import cats.implicits.*
import edomata.backend.cqrs.FakeRepository.Saved
import edomata.core.*
import edomata.syntax.all.*
import munit.CatsEffectAssertions.*

final class FakeRepository[State, Event](
    state: AggregateState[State],
    _saved: Ref[IO, List[Saved[State, Event]]]
) extends Repository[IO, State, Event] {

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
      Saved(
        ctx,
        version,
        newState,
        events
      )
    )
  )

  def saved: IO[List[Saved[State, Event]]] = _saved.get
  def assert(item: Saved[State, Event]): IO[Unit] =
    saved.assertEquals(List(item))
}

object FakeRepository {
  def apply[S, E](
      state: AggregateState[S]
  ): IO[FakeRepository[S, E]] =
    IO.ref(List.empty[Saved[S, E]])
      .map(new FakeRepository(state, _))

  final case class Saved[State, Event](
      ctx: CommandMessage[?],
      version: SeqNr,
      newState: State,
      events: Chain[Event]
  )

}
