package edomata.backend.rev2

import cats.Monad
import cats.data.EitherNec
import cats.data.NonEmptyChain
import cats.effect.kernel.Resource
import edomata.core.CommandMessage
import edomata.core.Model
import fs2.Stream

import java.time.OffsetDateTime
import edomata.core.CommandHandler

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

trait CommandStore2[F[_], M] {
  def append(cmd: CommandMessage[?, M]): F[Unit]
  def contains(id: String): F[Boolean]
}

trait DomainBackend[F[_], C, S, E, R, N, M] {
  val outbox: Outbox[F, N]
  val commands: CommandStore2[F, M]
  val journal: Journal[F, E]
  val handler: CommandHandler[F, C, S, E, R, N, M]
}
