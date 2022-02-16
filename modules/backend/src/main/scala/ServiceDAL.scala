package edomata.backend

import cats.Show
import cats.data.NonEmptyChain
import cats.effect.*
import cats.effect.implicits.*
import cats.implicits.*
import edomata.backend.CommandMessage
import edomata.eventsourcing.*
import io.circe.Codec
import io.circe.Decoder
import io.circe.Encoder
import io.odin.Logger
import io.odin.syntax.*

import java.time.OffsetDateTime

import FSMDefinition.*

/** Data access layer interface
  */
trait ServiceDAL[F[_], Domain] {

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
      version: EventVersion,
      events: NonEmptyChain[InternalEventFor[Domain]]
  ): F[Unit]

  /** Reads stream state from journal, implementations can decide whether or not
    * use snapshoting.
    *
    * @param streamId
    *   id of the stream to read
    */
  def readFromJournal(streamId: String): F[AggregateState[StateFor[Domain]]]

  /** Write a message to outbox
    *
    * @param msg
    *   message to outbox
    * @param time
    *   time of outboxing
    * @return
    *   aggregate state for stream
    */
  def outbox(msg: ExternalEventFor[Domain], time: OffsetDateTime): F[Unit]

  /** Append a command for idempotency check
    *
    * @param cmd
    *   domain command to append
    */
  def appendCmdLog(cmd: DomainCommand[Domain]): F[Unit]

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
  def transaction[T](f: F[T]): F[T]
}
