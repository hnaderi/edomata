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
