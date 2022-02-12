package edfsm.backend.skunk

import cats.Show
import cats.data.NonEmptyChain
import cats.effect.*
import cats.effect.implicits.*
import cats.implicits.*
import edfsm.backend.CommandMessage
import edfsm.backend.CommandStore
import edfsm.backend.DomainCommand
import edfsm.backend.DomainTransition
import edfsm.backend.FSMDefinition.*
import edfsm.backend.ServiceDAL
import edfsm.eventsourcing.*
import io.circe.Codec
import io.circe.Decoder
import io.circe.Encoder
import io.odin.Logger
import io.odin.syntax.*
import skunk.Session

import java.time.OffsetDateTime

object PGServiceDAL {
  def from[F[_]: Async, Domain](
      name: String,
      initial: StateFor[Domain],
      fold: DomainTransition[Domain],
      logger: Logger[F]
  )(pool: Resource[F, Session[F]])(using
      Encoder[CommandFor[Domain]],
      Encoder[StateFor[Domain]],
      Decoder[StateFor[Domain]],
      Encoder[InternalEventFor[Domain]],
      Decoder[InternalEventFor[Domain]],
      Encoder[ExternalEventFor[Domain]],
      Decoder[ExternalEventFor[Domain]]
  ): Resource[F, ServiceDAL[F, Domain]] =
    for {
      session <- pool
      no <- Resource.eval(
        PGOutboxService[F, ExternalEventFor[Domain]](
          session,
          name,
          logger
        )
      )
    } yield new ServiceDAL[F, Domain] {
      given Show[RejectionFor[Domain]] = Show.fromToString
      val log = logger.withConstContext(Map("service" -> name))
      val journal = PGJournal[F, InternalEventFor[Domain]](name, session)

      def repo: ESRepository[F, String, InternalEventFor[Domain], StateFor[
        Domain
      ]] =
        MyESRepository(
          snapshot = snapshot,
          journal = journal,
          logger = logger,
          initial = initial,
          fold = fold
        )
      def notification: OutboxService[F, ExternalEventFor[Domain]] = no
      def commands: CommandStore[F, CommandFor[Domain]] =
        PGCommandStore[F, CommandFor[Domain]](session)
      def snapshot: SnapshotStore[F, String, AggregateState[StateFor[Domain]]] =
        PGSnapshotStore[F, StateFor[Domain]](name, session)

      def transaction[T](f: F[T]): F[T] = session.transaction.use(_ => f)

      def appendJournal(
          streamId: String,
          time: OffsetDateTime,
          version: EventVersion,
          events: NonEmptyChain[InternalEventFor[Domain]]
      ): F[Unit] = repo.append(streamId, time, version, events)

      def readFromJournal(
          streamId: String
      ): F[AggregateState[StateFor[Domain]]] = repo.get(streamId)
      def outbox(t: ExternalEventFor[Domain], time: OffsetDateTime): F[Unit] =
        notification.outbox(t, time)
      def appendCmdLog(cmd: DomainCommand[Domain]): F[Unit] =
        commands.append(cmd)
      def containsCmd(id: String): F[Boolean] = commands.contains(id)
    }

  def setup[F[_], Domain](
      name: String,
      logger: Logger[F]
  )(session: Session[F])(using
      MonadCancel[F, Throwable],
      Encoder[ExternalEventFor[Domain]],
      Decoder[ExternalEventFor[Domain]]
  ): F[Unit] = {
    logger.info("Setting up persistence", Map("service" -> name)) >>
      session.transaction.use(_ =>
        PGJournal.setup(name, session) >>
          PGSnapshotStore.setup(name, session) >>
          PGOutboxService.setup[F, ExternalEventFor[Domain]](
            session,
            name
          ) >>
          PGCommandStore.setup(session)
      )
  }
}
