package edomata.backend

import cats.data.NonEmptyChain
import edomata.core.*
import fs2.Stream

import java.time.OffsetDateTime
import java.util.UUID

trait Outbox[F[_], T] extends OutboxReader[F, T], OutboxWriter[F, T]

trait OutboxWriter[F[_], T] {
  def write(t: T, time: OffsetDateTime): F[Unit]
}
trait OutboxReader[F[_], T] {
  def read: Stream[F, OutboxItem[T]]
  def markAllAsSent(items: NonEmptyChain[OutboxItem[T]]): F[Unit]
  def markAsSent(item: OutboxItem[T], others: OutboxItem[T]*): F[Unit] =
    markAllAsSent(NonEmptyChain.of(item, others: _*))
}

final case class OutboxItem[T](
    seqNr: SeqNr,
    time: OffsetDateTime,
    data: T
)
