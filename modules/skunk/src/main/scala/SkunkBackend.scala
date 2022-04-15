package edomata.backend

import cats.effect.Concurrent
import cats.effect.kernel.Resource
import edomata.core.Domain
import edomata.core.ModelTC
import skunk.Session

final class SkunkBackend[F[_]: Concurrent, S, E, R, N] private (
    _journal: Journal[F, E],
    _outbox: Outbox[F, N],
    snapshot: SnapshotStore[F, S, E, R],
    command: CommandStore[F]
)(using m: edomata.core.ModelTC[S, E, R])
    extends Backend[F, S, E, R, N] {
  def compiler[C]: edomata.core.Compiler[F, C, S, E, R, N] = ???

  val outbox: OutboxReader[F, N] = _outbox
  val journal: JournalReader[F, E] = _journal
  val repository: Repository[F, S, E, R] = Repository(journal, snapshot)
}

object SkunkBackend {
  def of[S, N]: Builder[S, N] = Builder()

  private[backend] final class Builder[S, N](
      private val dummy: Boolean = true
  ) extends AnyVal {

    def build[F[_]: Concurrent, E, R](pool: Resource[F, Session[F]])(using
        m: ModelTC[S, E, R]
    ): SkunkBackend[F, S, E, R, N] = ???
  }

}

extension [C, S, E, R, N](domain: Domain[C, S, E, R, N]) {
  def skunkBackend[F[_]: Concurrent](pool: Resource[F, Session[F]])(using
      m: ModelTC[S, E, R]
  ): SkunkBackend[F, S, E, R, N] = ???
}
