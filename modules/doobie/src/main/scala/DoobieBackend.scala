package edomata.backend

import cats.data.EitherNec
import cats.data.NonEmptyChain
import cats.effect.Concurrent
import cats.effect.kernel.Clock
import cats.effect.kernel.Resource
import cats.implicits.*
import doobie.ConnectionIO
import doobie.FC
import doobie.Transactor
import doobie.implicits.*
import edomata.core.CommandMessage
import edomata.core.Compiler
import edomata.core.Domain
import edomata.core.DomainModel
import edomata.core.ModelTC
import edomata.core.ProgramResult
import edomata.core.RequestContext
import fs2.Stream

import java.time.OffsetDateTime
import java.time.ZoneOffset

private val void: EitherNec[Nothing, Unit] = Right(())

final class DoobieBackend[F[_], S, E, R, N] private (
    _journal: Journal[ConnectionIO, E],
    _outbox: Outbox[ConnectionIO, N],
    snapshot: SnapshotStore[F, S, E, R],
    command: CommandStore[ConnectionIO],
    trx: Transactor[F]
)(using m: ModelTC[S, E, R], F: Concurrent[F], clock: Clock[F])
    extends Backend[F, S, E, R, N] {

  private def publish(
      notifs: Seq[N],
      cmd: CommandMessage[?],
      time: OffsetDateTime
  ): F[EitherNec[R, Unit]] =
    NonEmptyChain
      .fromSeq(notifs)
      .fold(void.pure)(
        _outbox.write(_, time, cmd.metadata).transact(trx).as(void)
      )

  private def publishCIO(
      notifs: Seq[N],
      cmd: CommandMessage[?],
      time: OffsetDateTime
  ): ConnectionIO[Unit] =
    NonEmptyChain
      .fromSeq(notifs)
      .fold(FC.unit)(
        _outbox.write(_, time, cmd.metadata)
      )

  private def currentTime =
    clock.realTimeInstant.map(_.atOffset(ZoneOffset.UTC))

  def compiler[C]: Compiler[F, C, S, E, R, N] = new {
    def onRequest(cmd: CommandMessage[C])(
        run: RequestContext[C, S] => F[ProgramResult[S, E, R, N]]
    ): F[EitherNec[R, Unit]] =
      currentTime.flatMap { now =>
        repository.get(cmd.id).flatMap {
          case AggregateState.Valid(s, version) =>
            run(cmd.buildContext(s)).flatMap {
              case ProgramResult.Accepted(ns, evs, notifs) =>
                (_journal.append(streamId = cmd.address, now, version, evs) >>
                  publishCIO(notifs, cmd, now) >>
                  command.append(cmd)).transact(trx).as(void)
              case ProgramResult.Indecisive(notifs) =>
                publish(notifs, cmd, now)
              case ProgramResult.Rejected(notifs, errs) =>
                publish(notifs, cmd, now).as(errs.asLeft)
              case ProgramResult.Conflicted(errs) => errs.asLeft.pure
            }
          case AggregateState.Conflicted(ls, ev, errs) =>
            errs.asLeft.pure
        }
      }
  }

  lazy val outbox: OutboxReader[F, N] = DoobieOutboxReader(trx, _outbox)
  lazy val journal: JournalReader[F, E] = DoobieJournalReader(trx, _journal)
  lazy val repository: Repository[F, S, E, R] = Repository(journal, snapshot)
}

private final class DoobieJournalReader[F[_]: Concurrent, E](
    trx: Transactor[F],
    reader: JournalReader[ConnectionIO, E]
) extends JournalReader[F, E] {
  def readStream(streamId: StreamId): Stream[F, EventMessage[E]] =
    reader.readStream(streamId).transact(trx)
  def readStreamAfter(
      streamId: StreamId,
      version: EventVersion
  ): Stream[F, EventMessage[E]] =
    reader.readStreamAfter(streamId, version).transact(trx)

  def readStreamBefore(
      streamId: StreamId,
      version: EventVersion
  ): Stream[F, EventMessage[E]] =
    reader.readStreamBefore(streamId, version).transact(trx)

  def readAll: Stream[F, EventMessage[E]] = reader.readAll.transact(trx)
  def readAllAfter(seqNr: SeqNr): Stream[F, EventMessage[E]] =
    reader.readAllAfter(seqNr).transact(trx)
  def notifications: Stream[F, StreamId] =
    ??? // TODO how to implement the doobie version?
}

private final class DoobieOutboxReader[F[_]: Concurrent, N](
    trx: Transactor[F],
    reader: OutboxReader[ConnectionIO, N]
) extends OutboxReader[F, N] {
  def read: Stream[F, OutboxItem[N]] = reader.read.transact(trx)
  def markAllAsSent(items: NonEmptyChain[OutboxItem[N]]): F[Unit] =
    reader.markAllAsSent(items).transact(trx)
}

object DoobieBackend {

  def apply[F[_]: Concurrent](): Builder[F] = Builder()

  final class Builder[F[_]: Concurrent] {

    def build[C, S, E, R, N](
        domain: Domain[C, S, E, R, N],
        namespace: String
    )(using
        m: ModelTC[S, E, R]
    ): F[DoobieBackend[F, S, E, R, N]] = ???

    def buildNoSetup[C, S, E, R, N](
        domain: Domain[C, S, E, R, N],
        namespace: String
    )(using
        m: ModelTC[S, E, R]
    ): DoobieBackend[F, S, E, R, N] = ???
  }

}
