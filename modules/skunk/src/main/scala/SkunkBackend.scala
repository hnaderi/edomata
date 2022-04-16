package edomata.backend

import cats.data.NonEmptyChain
import cats.effect.Concurrent
import cats.effect.kernel.Clock
import cats.effect.kernel.Resource
import edomata.core.Compiler
import edomata.core.Domain
import edomata.core.ModelTC
import fs2.Stream
import skunk.Session

final class SkunkBackend[F[_], S, E, R, N] private (
    snapshot: SnapshotStore[F, S, E, R],
    pool: Resource[F, Session[F]]
)(using m: ModelTC[S, E, R], F: Concurrent[F], clock: Clock[F])
    extends Backend[F, S, E, R, N] {
  def compiler[C]: Compiler[F, C, S, E, R, N] = ???

  val outbox: OutboxReader[F, N] = SkunkOutboxReader(pool)
  val journal: JournalReader[F, E] = SkunkJournalReader(pool)
  val repository: Repository[F, S, E, R] = Repository(journal, snapshot)
}

private final class SkunkJournalReader[F[_]: Concurrent, E](
    pool: Resource[F, Session[F]]
) extends JournalReader[F, E] {
  def readStream(streamId: StreamId): Stream[F, EventMessage[E]] = ???
  def readStreamAfter(
      streamId: StreamId,
      version: EventVersion
  ): Stream[F, EventMessage[E]] = ???
  def readStreamBefore(
      streamId: StreamId,
      version: EventVersion
  ): Stream[F, EventMessage[E]] = ???
  def readAll: Stream[F, EventMessage[E]] = ???
  def readAllAfter(seqNr: SeqNr): Stream[F, EventMessage[E]] = ???
  def notifications: Stream[F, StreamId] = ???
}

private final class SkunkOutboxReader[F[_]: Concurrent, N](
    pool: Resource[F, Session[F]]
) extends OutboxReader[F, N] {
  def read: Stream[F, OutboxItem[N]] = ???
  def markAllAsSent(items: NonEmptyChain[OutboxItem[N]]): F[Unit] = ???
}

object SkunkBackend {
  def apply[F[_]: Concurrent](pool: Resource[F, Session[F]]): Builder[F] =
    Builder(pool)

  final class Builder[F[_]: Concurrent](pool: Resource[F, Session[F]]) {
    def build[C, S, E, R, N](domain: Domain[C, S, E, R, N], namespace: String)(
        using m: ModelTC[S, E, R]
    ): Resource[F, SkunkBackend[F, S, E, R, N]] = ???

    def buildUnsafe[C, S, E, R, N](
        domain: Domain[C, S, E, R, N],
        namespace: String
    )(using
        m: ModelTC[S, E, R]
    ): SkunkBackend[F, S, E, R, N] = ???
  }

}
