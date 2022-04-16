package edomata.backend

import cats.data.NonEmptyChain
import edomata.core.*
import fs2.Stream

import java.time.OffsetDateTime
import java.util.UUID

type SeqNr = Long
type EventVersion = Long
type StreamId = String

trait Backend[F[_], S, E, R, N] {
  def compiler[C]: edomata.core.Compiler[F, C, S, E, R, N]
  lazy val outbox: OutboxReader[F, N]
  lazy val journal: JournalReader[F, E]
  lazy val repository: Repository[F, S, E, R]
}
