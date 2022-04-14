package edomata.backend

import cats.data.NonEmptyChain
import edomata.core.*
import fs2.Stream

import java.time.OffsetDateTime
import java.util.UUID

type SeqNr = Long
type EventVersion = Long
type StreamId = String

final case class Backend[F[_], C, S, E, R, N](
    journal: JournalReader[F, E],
    outbox: OutboxReader[F, N],
    repository: Repository[F, S, E, R],
    compiler: Compiler[F, C, S, E, R, N]
)

object Backend {
  import Domain.*
  type Of[F[_], D] = Backend[
    F,
    CommandFor[D],
    StateFor[D],
    EventFor[D],
    RejectionFor[D],
    NotificationFor[D]
  ]
}
