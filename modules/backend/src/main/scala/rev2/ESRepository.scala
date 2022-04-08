package edomata.backend.rev2

import cats.Monad
import cats.data.EitherNec
import cats.data.NonEmptyChain
import cats.effect.Concurrent
import cats.effect.kernel.Resource
import edomata.core.CommandMessage
import edomata.core.Model
import fs2.Chunk
import fs2.Pipe
import fs2.Stream

import java.time.OffsetDateTime

trait ESRepository[F[_], S, E, R] extends Projection[F, S, E, R] {
  def append(
      streamId: StreamId,
      time: OffsetDateTime,
      version: EventVersion,
      events: NonEmptyChain[E]
  ): F[Unit]
}

object ESRepository {
  import cats.implicits.*

  def noSnapshots[F[_]: Concurrent, S, E, R](
      initial: Model.Of[S, E, R],
      journal: Journal[F, E]
  ): ESRepository[F, Model.Of[S, E, R], E, R] =
    new ESRepositoryNoSnapshot(initial, journal)

  def withSnapshot[F[_]: Concurrent, S, E, R](
      initial: Model.Of[S, E, R],
      journal: Journal[F, E],
      snapshot: SnapshotStore[F, Model.Of[S, E, R]]
  ): ESRepository[F, Model.Of[S, E, R], E, R] =
    new ESRepositoryWithSnapshot(initial, journal, snapshot)

  final class ESRepositoryNoSnapshot[F[_]: Concurrent, S, E, R](
      initial: Model.Of[S, E, R],
      journal: Journal[F, E]
  ) extends ESRepositoryBase[F, S, E, R](initial, journal) {
    def get(
        streamId: StreamId
    ): F[AggregateState[Model.Of[S, E, R], E, R]] =
      journal
        .readStream(streamId)
        .through(foldPipe(initialAggState))
        .compile
        .lastOrError

  }

  final class ESRepositoryWithSnapshot[F[_]: Concurrent, S, E, R](
      initial: Model.Of[S, E, R],
      journal: Journal[F, E],
      snapshot: SnapshotStore[F, Model.Of[S, E, R]]
  ) extends ESRepositoryBase[F, S, E, R](initial, journal) {
    private val F = Concurrent[F]

    def get(
        streamId: StreamId
    ): F[AggregateState[Model.Of[S, E, R], E, R]] =
      snapshot
        .get(streamId)
        .flatMap { lastOpt =>
          val last = lastOpt.getOrElse(initialAggState)
          journal
            .readAllAfter(last.version)
            .through(foldPipe(last))
            .compile
            .lastOrError
        }
        .flatTap(_.fold(snapshot.put(streamId, _), _ => F.unit))
  }

  abstract class ESRepositoryBase[F[_], S, E, R](
      initial: Model.Of[S, E, R],
      journal: Journal[F, E]
  ) extends ESRepository[F, Model.Of[S, E, R], E, R] {
    protected val initialAggState = AggregateState.Valid(0, initial)

    def append(
        streamId: StreamId,
        time: OffsetDateTime,
        version: EventVersion,
        events: NonEmptyChain[E]
    ): F[Unit] = journal.append(streamId, time, version, events)
    def history(
        streamId: StreamId
    ): Stream[F, AggregateState[Model.Of[S, E, R], E, R]] = journal
      .readStream(streamId)
      .through(scanPipe(initialAggState))
  }

  private def fold[S, E, R](
      v: Long,
      s: Model.Of[S, E, R],
      e: EventMessage[E]
  ): AggregateState[Model.Of[S, E, R], E, R] =
    s.transition(e.payload)
      .fold(
        errs => AggregateState.Failed(s, e, errs),
        ns => AggregateState.Valid(e.metadata.version, ns)
      )

  private def scanPipe[F[_], S, E, R](
      init: AggregateState[Model.Of[S, E, R], E, R]
  ): Pipe[F, EventMessage[E], AggregateState[Model.Of[S, E, R], E, R]] =
    _.scan(init) {
      case (AggregateState.Valid(v, s), e) =>
        fold(v, s, e)
      case (other, e) =>
        other
    }
      .takeWhile(_.isValid, true)

  private def foldPipe[F[_], S, E, R](
      init: AggregateState[Model.Of[S, E, R], E, R]
  ): Pipe[F, EventMessage[E], AggregateState[Model.Of[S, E, R], E, R]] =
    _.fold(init) {
      case (AggregateState.Valid(v, s), e) =>
        fold(v, s, e)
      case (other, e) =>
        other
    }
      .takeWhile(_.isValid, true)
}
