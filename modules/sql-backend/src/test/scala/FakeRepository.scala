package edomata.backend

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
      notifications: Seq[N]
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
        notifications: Seq[N]
    )
    case Notified(
        ctx: RequestContext[?, ?],
        notifications: NonEmptyChain[N]
    )
  }
}
