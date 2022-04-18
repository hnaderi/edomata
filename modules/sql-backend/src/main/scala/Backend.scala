package edomata.backend

import cats.data.EitherNec
import edomata.core.*

trait Backend[F[_], S, E, R, N] {
  def compile[C](
      app: Edomaton[F, RequestContext[C, S], R, E, N, Unit]
  ): DomainService[F, CommandMessage[C], R]
  val outbox: OutboxReader[F, N]
  val journal: JournalReader[F, E]
  val repository: RepositoryReader[F, S, E, R]
}
