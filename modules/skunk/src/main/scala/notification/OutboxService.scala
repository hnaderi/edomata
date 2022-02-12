package edfsm.backend.skunk

import java.time.OffsetDateTime
import fs2.Stream

trait OutboxService[F[_], T] {
  def outbox(t: T, time: OffsetDateTime): F[Unit]
}

final case class OutboxItem[T](
    seqNr: Long,
    time: OffsetDateTime,
    data: T
)
