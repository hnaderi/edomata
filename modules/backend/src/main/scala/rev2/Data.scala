package edomata.backend.rev2

import java.time.OffsetDateTime
import java.util.UUID

type SeqNr = Long
type EventVersion = Long
type StreamId = String
type ConsumerName = String

final case class EventMetadata(
    id: UUID,
    time: OffsetDateTime,
    seqNr: SeqNr,
    version: EventVersion,
    stream: String
)

final case class EventMessage[+T](metadata: EventMetadata, payload: T)

final case class AggregateState[T](version: Long, state: T)
