package edomata.eventsourcing

import fs2.Stream
import cats.data.NonEmptyChain
import java.time.OffsetDateTime
import java.util.UUID

trait Journal[F[_], I, E] {
  def append(
      streamId: I,
      time: OffsetDateTime,
      version: SeqNr,
      events: NonEmptyChain[E]
  ): F[Unit]
  def readStream(streamId: I): Stream[F, EventMessage[E]]
  def readStreamAfter(
      streamId: I,
      version: EventVersion
  ): Stream[F, EventMessage[E]]
  def readAll: Stream[F, EventMessage[E]]
  def readAllAfter(seqNr: SeqNr): Stream[F, EventMessage[E]]
  def notifications: Stream[F, I]
}
