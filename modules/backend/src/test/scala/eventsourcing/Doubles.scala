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
import edomata.backend.eventsourcing.AggregateState.Valid
import edomata.core.*
import fs2.Chunk

final class BlackHoleSnapshotStore[S] extends SnapshotStore[IO, S] {
  def put(id: StreamId, state: AggregateState.Valid[S]): IO[Unit] =
    IO.unit
  def get(id: StreamId): IO[Option[AggregateState.Valid[S]]] = IO(None)
  def getFast(id: StreamId): IO[Option[AggregateState.Valid[S]]] = IO(
    None
  )
}

final class ConstantSnapshotStore[S](state: S, version: SeqNr)
    extends SnapshotStore[IO, S] {
  def put(id: StreamId, state: AggregateState.Valid[S]): IO[Unit] =
    IO.unit
  def get(id: StreamId): IO[Option[AggregateState.Valid[S]]] = IO(
    Some(AggregateState.Valid(state, version))
  )
  def getFast(id: StreamId): IO[Option[AggregateState.Valid[S]]] = IO(
    Some(AggregateState.Valid(state, version))
  )
}

final class LaggedSnapshotStore[S](state: S, version: SeqNr, lagged: Long)
    extends SnapshotStore[IO, S] {
  def put(id: StreamId, state: AggregateState.Valid[S]): IO[Unit] =
    IO.unit
  def get(id: StreamId): IO[Option[AggregateState.Valid[S]]] = IO(
    Some(AggregateState.Valid(state, lagged))
  )
  def getFast(id: StreamId): IO[Option[AggregateState.Valid[S]]] = IO(
    Some(AggregateState.Valid(state, version))
  )
}

final class FakeSnapShotStore[S](
    states: Ref[IO, Map[StreamId, AggregateState.Valid[S]]]
) extends SnapshotStore[IO, S] {
  def put(id: StreamId, state: AggregateState.Valid[S]): IO[Unit] =
    states.update(_.updated(id, state))
  def get(id: StreamId): IO[Option[AggregateState.Valid[S]]] =
    states.get.map(_.get(id))
  def getFast(id: StreamId): IO[Option[AggregateState.Valid[S]]] =
    states.get.map(_.get(id))

  def all: IO[Map[StreamId, Valid[S]]] = states.get
}

object FakeSnapShotStore {
  def apply[S](): IO[FakeSnapShotStore[S]] =
    IO.ref(Map.empty[StreamId, Valid[S]]).map(new FakeSnapShotStore(_))
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

class FakeSnapshotPersistence[S](
    ref: Ref[IO, Map[StreamId, AggregateState.Valid[S]]]
) extends SnapshotPersistence[IO, S] {
  def get(id: StreamId): IO[Option[AggregateState.Valid[S]]] =
    ref.get.map(_.get(id))
  def put(items: Chunk[SnapshotItem[S]]): IO[Unit] =
    ref.update(_ ++ items.iterator)
}
object FakeSnapshotPersistence {
  def apply[S]: IO[FakeSnapshotPersistence[S]] = IO
    .ref(Map.empty[StreamId, AggregateState.Valid[S]])
    .map(new FakeSnapshotPersistence(_))
}

class FailingSnapshotPersistence[S](
    repo: SnapshotPersistence[IO, S],
    failures: Ref[IO, Int],
    reqFailures: Int
) extends SnapshotPersistence[IO, S] {
  val pred = failures.get.map(_ < reqFailures)
  def run[T](f: IO[T]): IO[T] = pred
    .ifM(
      failures.update(_ + 1) >> IO.raiseError(PlanedFailure),
      f
    )

  def get(id: StreamId): IO[Option[AggregateState.Valid[S]]] =
    run(repo.get(id))

  def put(items: Chunk[SnapshotItem[S]]): IO[Unit] =
    run(repo.put(items))
}
object FailingSnapshotPersistence {
  def apply[S](reqFailures: Int): IO[FailingSnapshotPersistence[S]] =
    FakeSnapshotPersistence[S]
      .product(IO.ref(0))
      .map(new FailingSnapshotPersistence(_, _, reqFailures))
}

object PlanedFailure extends Throwable
