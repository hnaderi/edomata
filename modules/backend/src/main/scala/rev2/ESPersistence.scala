package edomata.backend.rev2

import cats.data.EitherNec
import cats.data.NonEmptyChain
import edomata.core.CommandMessage
import edomata.core.Model
import fs2.Stream

import java.time.OffsetDateTime
import cats.effect.kernel.Resource
import cats.Monad

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
  def readFromJournal(streamId: String): F[EitherNec[R, AggregateState[S]]]

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

  def apply[F[_]: Monad, S, E, R](
      initial: S & Model[S, E, R],
      journal: Journal[F, E]
  ): ESRepository[F, S & Model[S, E, R], E, R] = new ESRepository {
    def append(
        streamId: StreamId,
        time: OffsetDateTime,
        version: EventVersion,
        events: NonEmptyChain[E]
    ): F[Unit] = journal.append(streamId, time, version, events)
    def get(
        streamId: StreamId
    ): F[EitherNec[R, AggregateState[S & Model[S, E, R]]]] = ???

    def history: Stream[F, AggregateState[S & Model[S, E, R]]] = journal.readAll
      .scan(AggregateState(0, initial).asRight[NonEmptyChain[R]]) {
        case (Right(s), e) =>
          s.state
            .transition(e.payload)
            .toEither
            .map(newS => s.copy(state = newS, version = s.version + 1))
        case (other, _) => other
      }
      .flatMap {
        case Left(r)  => Stream.empty
        case Right(s) => Stream.emit(s)
      }
    def at(
        version: EventVersion
    ): F[Option[AggregateState[S & Model[S, E, R]]]] = ???
  }
}

trait Projection[F[_], P, E, R] {
  def get(streamId: StreamId): F[EitherNec[R, AggregateState[P]]]
  def history: Stream[F, AggregateState[P]]
  def at(version: EventVersion): F[Option[AggregateState[P]]]
}

trait SnapshotStore[F[_], S] {
  def get(id: StreamId): F[Option[S]]
  def put(id: StreamId, state: S): F[Unit]
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
