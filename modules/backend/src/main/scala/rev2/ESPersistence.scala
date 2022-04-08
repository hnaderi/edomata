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

/** Data access layer interface
  */
trait ESPersistence[F[_], S, E, R, N, M] {

  /** Appends all events to journal or raises error otherwise Guaranties:
    *   - stream version = version
    *   - event times = time
    *   - events are foldable according to current state
    *
    * @param streamId
    *   id of the stream to append to
    * @param time
    *   time of insertion
    * @param version
    *   version of stream that events are based on
    * @param events
    *   events to append
    */
  def appendJournal(
      streamId: String,
      time: OffsetDateTime,
      version: Long,
      events: NonEmptyChain[E]
  ): F[Unit]

  /** Reads stream state from journal, implementations can decide whether or not
    * use snapshoting.
    *
    * @param streamId
    *   id of the stream to read
    */
  def readFromJournal(streamId: String): F[AggregateState[S, E, R]]

  /** Write messages to outbox
    *
    * @param msg
    *   message to outbox
    * @param time
    *   time of outboxing
    * @return
    *   aggregate state for stream
    */
  def outbox(msgs: NonEmptyChain[N]): F[Unit]

  /** Append a command for idempotency check
    *
    * @param cmd
    *   domain command to append
    */
  def appendCmdLog(cmd: CommandMessage[?, M]): F[Unit]

  /** Whether or not this command id exists
    *
    * @param id
    *   command id
    * @return
    *   true if exists
    */
  def containsCmd(id: String): F[Boolean]

  /** run given program in transaction
    *
    * @param f
    *   program to execute in transaction
    * @return
    *   program that runs given program in trasaction
    */
  def transaction: Resource[F, Unit]
}

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

trait Projection[F[_], P, E, R] {
  def get(streamId: StreamId): F[AggregateState[P, E, R]]
  def history(streamId: StreamId): Stream[F, AggregateState[P, E, R]]
}

trait SnapshotStore[F[_], S] {
  def get(id: StreamId): F[Option[AggregateState.Valid[S]]]
  def put(id: StreamId, state: AggregateState.Valid[S]): F[Unit]
}

trait Journal[F[_], E] {
  def append(
      streamId: StreamId,
      time: OffsetDateTime,
      version: SeqNr,
      events: NonEmptyChain[E]
  ): F[Unit]
  def readStream(streamId: StreamId): Stream[F, EventMessage[E]]
  def readStreamAfter(
      streamId: StreamId,
      version: EventVersion
  ): Stream[F, EventMessage[E]]
  def readAll: Stream[F, EventMessage[E]]
  def readAllAfter(seqNr: SeqNr): Stream[F, EventMessage[E]]
  def notifications: Stream[F, Unit]
}

trait ConsumerController[F[_]] {
  def seek(name: ConsumerName, seqNr: SeqNr): F[Unit]
  def read(name: ConsumerName): F[Option[SeqNr]]
}

trait CommandStore2[F[_], M] {
  def append(cmd: CommandMessage[?, M]): F[Unit]
  def contains(id: String): F[Boolean]
}
