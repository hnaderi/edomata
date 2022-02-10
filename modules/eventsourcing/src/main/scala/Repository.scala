package edfsm.eventsourcing

import fs2.Stream
import cats.data.NonEmptyChain
import java.time.OffsetDateTime
import java.util.UUID

type SeqNr = Long
type EventVersion = Long

trait Repository[F[_], I, E, S] {
  def append(
      streamId: I,
      time: OffsetDateTime,
      version: EventVersion,
      events: NonEmptyChain[E]
  ): F[Unit]
  def get(streamId: I): F[AggregateState[S]]
}

final case class EventMetadata(
    id: UUID,
    time: OffsetDateTime,
    seqNr: SeqNr,
    version: EventVersion,
    stream: String
)

final case class EventMessage[+T](metadata: EventMetadata, payload: T)

final case class AggregateState[T](version: Long, state: T)
