package edomata.backend

import cats.data.EitherNec
import cats.data.NonEmptyChain
import cats.implicits.*
import edomata.core.CommandMessage

import java.time.OffsetDateTime
import edomata.eventsourcing.AggregateState

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

  /** Write a message to outbox
    *
    * @param msg
    *   message to outbox
    * @param time
    *   time of outboxing
    * @return
    *   aggregate state for stream
    */
  def outbox(msgs: Seq[N]): F[Unit]

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
  def transaction[T](f: F[T]): F[T]
}
