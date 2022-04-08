package edomata.backend.rev2

import cats.Monad
import cats.data.EitherNec
import cats.data.NonEmptyChain
import cats.effect.kernel.Resource
import edomata.core.CommandMessage
import edomata.core.Model
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

  /** transaction with read committed isolation
    *
    * @return
    *   program that runs given program in trasaction
    */
  def transaction: Resource[F, Unit]
}

object ESPersistence {
  def apply[F[_], S, E, R, N, M](
      trx: Resource[F, Unit],
      repo: ESRepository[F, Model.Of[S, E, R], E, R],
      cmdStore: CommandStore2[F, M],
      notifications: Outbox[F, N]
  ): ESPersistence[F, Model.Of[S, E, R], E, R, N, M] =
    new ESPersistence {
      def appendJournal(
          streamId: String,
          time: OffsetDateTime,
          version: Long,
          events: NonEmptyChain[E]
      ): F[Unit] = repo.append(streamId, time, version, events)

      def readFromJournal(
          streamId: String
      ): F[AggregateState[Model.Of[S, E, R], E, R]] =
        repo.get(streamId)

      def outbox(msgs: NonEmptyChain[N]): F[Unit] = notifications.outbox(msgs)

      def appendCmdLog(cmd: CommandMessage[?, M]): F[Unit] =
        cmdStore.append(cmd)

      def containsCmd(id: String): F[Boolean] = cmdStore.contains(id)

      def transaction: Resource[F, Unit] = trx
    }
}
