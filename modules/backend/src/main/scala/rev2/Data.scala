package edomata.backend.rev2

import cats.data.NonEmptyChain

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

import AggregateState.*
sealed trait AggregateState[+S, +E, +R] extends Product with Serializable {
  def isValid: Boolean
  def isFailed: Boolean
  def fold[T](fv: Valid[S] => T, ff: Failed[S, E, R] => T): T
}

object AggregateState {
  final case class Valid[+S](version: Long, state: S)
      extends AggregateState[S, Nothing, Nothing] {
    def isValid: Boolean = true
    def isFailed: Boolean = false
    def fold[T](fv: Valid[S] => T, ff: Failed[S, Nothing, Nothing] => T): T =
      fv(this)
  }

  final case class Failed[+S, +E, +R](
      lastState: S,
      onEvent: EventMessage[E],
      errors: NonEmptyChain[R]
  ) extends AggregateState[S, E, R] {
    def isValid: Boolean = false
    def isFailed: Boolean = true
    def fold[T](fv: Valid[S] => T, ff: Failed[S, E, R] => T): T = ff(this)
  }

  def valid[S, E, R](version: Long, state: S): AggregateState[S, E, R] =
    Valid(version, state)

  def failed[S, E, R](
      lastState: S,
      onEvent: EventMessage[E],
      errors: NonEmptyChain[R]
  ): AggregateState[S, E, R] = Failed(lastState, onEvent, errors)
}
