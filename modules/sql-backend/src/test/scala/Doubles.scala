package edomata.backend

import cats.data.Chain
import cats.data.NonEmptyChain
import cats.effect.IO
import cats.effect.kernel.Ref
import cats.implicits.*
import edomata.backend.AggregateState.Valid
import edomata.core.*

object BlackHoleCommandStore extends CommandStore[IO] {
  def append(cmd: CommandMessage[?]): IO[Unit] = IO.unit
  def contains(id: String): IO[Boolean] = IO(false)
}

object YesManCommandStore extends CommandStore[IO] {
  def append(cmd: CommandMessage[?]): IO[Unit] = IO.unit
  def contains(id: String): IO[Boolean] = IO(true)
}

final class BlackHoleSnapshotStore[S, E, R] extends SnapshotStore[IO, S, E, R] {
  def put(id: StreamId, state: AggregateState.Valid[S, E, R]): IO[Unit] =
    IO.unit
  def get(id: StreamId): IO[Option[AggregateState.Valid[S, E, R]]] = IO(None)
  def getFast(id: StreamId): IO[Option[AggregateState.Valid[S, E, R]]] = IO(
    None
  )
}

final class ConstantSnapshotStore[S, E, R](state: S, version: SeqNr)
    extends SnapshotStore[IO, S, E, R] {
  def put(id: StreamId, state: AggregateState.Valid[S, E, R]): IO[Unit] =
    IO.unit
  def get(id: StreamId): IO[Option[AggregateState.Valid[S, E, R]]] = IO(
    Some(AggregateState.Valid(state, version))
  )
  def getFast(id: StreamId): IO[Option[AggregateState.Valid[S, E, R]]] = IO(
    Some(AggregateState.Valid(state, version))
  )
}

final class LaggedSnapshotStore[S, E, R](state: S, version: SeqNr, lagged: Long)
    extends SnapshotStore[IO, S, E, R] {
  def put(id: StreamId, state: AggregateState.Valid[S, E, R]): IO[Unit] =
    IO.unit
  def get(id: StreamId): IO[Option[AggregateState.Valid[S, E, R]]] = IO(
    Some(AggregateState.Valid(state, lagged))
  )
  def getFast(id: StreamId): IO[Option[AggregateState.Valid[S, E, R]]] = IO(
    Some(AggregateState.Valid(state, version))
  )
}

final class FakeSnapShotStore[S, E, R](
    states: Ref[IO, Map[StreamId, AggregateState.Valid[S, E, R]]]
) extends SnapshotStore[IO, S, E, R] {
  def put(id: StreamId, state: AggregateState.Valid[S, E, R]): IO[Unit] =
    states.update(_.updated(id, state))
  def get(id: StreamId): IO[Option[AggregateState.Valid[S, E, R]]] =
    states.get.map(_.get(id))
  def getFast(id: StreamId): IO[Option[AggregateState.Valid[S, E, R]]] =
    states.get.map(_.get(id))

  def all: IO[Map[StreamId, Valid[S, E, R]]] = states.get
}

object FakeSnapShotStore {
  def apply[S, E, R](): IO[FakeSnapShotStore[S, E, R]] =
    IO.ref(Map.empty[StreamId, Valid[S, E, R]]).map(new FakeSnapShotStore(_))
}

final class FakeCommandStore(
    states: Ref[IO, Set[String]]
) extends CommandStore[IO] {
  def append(cmd: CommandMessage[?]): IO[Unit] = states.update(_ + cmd.id)
  def contains(id: String): IO[Boolean] = states.get.map(_.contains(id))
  def all: IO[Set[String]] = states.get
}

object FakeCommandStore {
  def apply(): IO[FakeCommandStore] =
    IO.ref(Set.empty[String]).map(new FakeCommandStore(_))
}

class FailingRepository[S, E, R, N] extends Repository[IO, S, E, R, N] {

  def load(cmd: CommandMessage[?]): IO[CommandState[S, E, R]] =
    IO.raiseError(PlanedFailure)

  def append(
      ctx: RequestContext[?, ?],
      version: SeqNr,
      newState: S,
      events: NonEmptyChain[E],
      notifications: Chain[N]
  ): IO[Unit] = IO.raiseError(PlanedFailure)

  def notify(
      ctx: RequestContext[?, ?],
      notifications: NonEmptyChain[N]
  ): IO[Unit] = IO.raiseError(PlanedFailure)
}

object PlanedFailure extends Throwable
