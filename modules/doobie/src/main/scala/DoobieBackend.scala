package edomata.backend

import cats.data.NonEmptyChain
import cats.effect.Concurrent
import cats.effect.kernel.Resource
import doobie.ConnectionIO
import doobie.Transactor
import doobie.implicits.*
import edomata.core.Domain
import edomata.core.ModelTC
import fs2.Stream

final class DoobieBackend[F[_]: Concurrent, S, E, R, N] private (
    _journal: Journal[ConnectionIO, E],
    _outbox: Outbox[ConnectionIO, N],
    snapshot: SnapshotStore[F, S, E, R],
    command: CommandStore[ConnectionIO],
    trx: Transactor[F]
)(using m: edomata.core.ModelTC[S, E, R])
    extends Backend[F, S, E, R, N] {
  def compiler[C]: edomata.core.Compiler[F, C, S, E, R, N] = ???

  val outbox: OutboxReader[F, N] = DoobieOutboxReader(trx, _outbox)
  val journal: JournalReader[F, E] = DoobieJournalReader(trx, _journal)
  val repository: Repository[F, S, E, R] = Repository(journal, snapshot)
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
  def readAll: Stream[F, EventMessage[E]] = reader.readAll.transact(trx)
  def readAllAfter(seqNr: SeqNr): Stream[F, EventMessage[E]] =
    reader.readAllAfter(seqNr).transact(trx)
  def notifications: Stream[F, StreamId] = ???
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
  def of[S, N]: Builder[S, N] = Builder()

  private[backend] final class Builder[S, N](
      private val dummy: Boolean = true
  ) extends AnyVal {

    def build[F[_]: Concurrent, E, R]()(using
        m: ModelTC[S, E, R]
    ): DoobieBackend[F, S, E, R, N] = ???
  }

}

extension [C, S, E, R, N](domain: Domain[C, S, E, R, N]) {
  def doobiePGBackend[F[_]: Concurrent](using
      m: ModelTC[S, E, R]
  ): DoobieBackend[F, S, E, R, N] = ???
}
