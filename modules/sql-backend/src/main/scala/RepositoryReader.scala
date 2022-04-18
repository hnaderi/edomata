package edomata.backend

import cats.data.NonEmptyChain
import cats.effect.Concurrent
import cats.implicits.*
import edomata.core.*
import fs2.Pipe
import fs2.Stream

import java.time.OffsetDateTime
import java.util.UUID

enum AggregateState[S, E, R](val isValid: Boolean) {
  case Valid(state: S, version: SeqNr) extends AggregateState[S, E, R](true)
  case Conflicted(
      last: S,
      onEvent: EventMessage[E],
      errors: NonEmptyChain[R]
  ) extends AggregateState[S, E, R](false)
}

trait RepositoryReader[F[_], S, E, R] {
  def get(streamId: StreamId): F[AggregateState[S, E, R]]
  def history(streamId: StreamId): Stream[F, AggregateState[S, E, R]]
}

object RepositoryReader {
  def apply[F[_], S, E, R](
      journal: JournalReader[F, E],
      snapshot: SnapshotReader[F, S, E, R]
  )(using F: Concurrent[F], m: ModelTC[S, E, R]): RepositoryReader[F, S, E, R] =
    new {
      def get(streamId: StreamId): F[AggregateState[S, E, R]] =
        snapshot
          .get(streamId)
          .flatMap {
            case Some(last) =>
              journal
                .readStreamAfter(streamId, last.version)
                .through(scanState(last))
                .compile
                .lastOrError
            case None => history(streamId).compile.lastOrError
          }

      def history(streamId: StreamId): Stream[F, AggregateState[S, E, R]] =
        journal
          .readStream(streamId)
          .through(scanState(AggregateState.Valid(m.initial, 0L)))
    }
}

private[backend] def scanState[F[_], S, E, R](
    last: AggregateState[S, E, R]
)(using
    m: ModelTC[S, E, R]
): Pipe[F, EventMessage[E], AggregateState[S, E, R]] =
  _.scan(last) {
    case (AggregateState.Valid(s, version), ev) =>
      m.transition(ev.payload)(s)
        .fold(
          AggregateState.Conflicted(s, ev, _),
          AggregateState.Valid(_, version + 1)
        )
    case (other, ev) => other
  }.takeWhile(_.isValid, true)
