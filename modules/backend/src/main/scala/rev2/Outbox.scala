package edomata.backend.rev2

import cats.data.NonEmptyChain
import fs2.Chunk
import fs2.Stream

import java.time.OffsetDateTime

final case class OutboxItem[T](
    seqNr: Long,
    time: OffsetDateTime,
    data: T
)

trait Outbox[F[_], N] {
  def outbox(notifications: NonEmptyChain[N]): F[Unit]
  def read: Stream[F, OutboxItem[N]]
  def markAsRead(item: OutboxItem[N]): F[Unit]
  def markChunkAsRead(chunk: Chunk[OutboxItem[N]]): F[Unit]
}
