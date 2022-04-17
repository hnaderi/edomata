package edomata.backend

import cats.data.EitherNec
import cats.data.NonEmptyChain
import cats.effect.Temporal
import cats.effect.Concurrent
import cats.effect.kernel.Clock
import cats.effect.kernel.Resource
import cats.implicits.*
import doobie.ConnectionIO
import doobie.FC
import doobie.Transactor
import doobie.implicits.*
import edomata.core.*
import fs2.Stream

import java.time.OffsetDateTime
import java.time.ZoneOffset

final class DoobieBackend[F[_], S, E, R, N] private (
    _journal: JournalReader[ConnectionIO, E],
    _outbox: OutboxReader[ConnectionIO, N],
    compiler: Compiler[F, E, N],
    snapshot: SnapshotStore[F, S, E, R],
    trx: Transactor[F]
)(using m: ModelTC[S, E, R], F: Temporal[F], clock: Clock[F])
    extends Backend[F, S, E, R, N](compiler) {
  lazy val outbox: OutboxReader[F, N] = DoobieOutboxReader(trx, _outbox)
  lazy val journal: JournalReader[F, E] = DoobieJournalReader(trx, _journal)
  lazy val repository: Repository[F, S, E, R] = Repository(journal, snapshot)
}

private final class DoobieCompiler[F[_], E, N](trx: Transactor[F])(using
    F: Concurrent[F],
    clock: Clock[F]
) extends Compiler[F, E, N] {
  def append(
      ctx: RequestContext[?, ?],
      version: SeqNr,
      events: NonEmptyChain[E],
      notifications: Seq[N]
  ): F[Unit] = ???

  def notify(
      ctx: RequestContext[?, ?],
      notifications: NonEmptyChain[N]
  ): F[Unit] = ???
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
