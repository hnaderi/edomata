package edomata.backend

import cats.data.EitherNec
import cats.data.NonEmptyChain
import cats.effect.Concurrent
import cats.effect.implicits.*
import cats.effect.kernel.Async
import cats.effect.kernel.Clock
import cats.effect.kernel.Resource
import cats.effect.kernel.Temporal
import cats.implicits.*
import edomata.core.*
import fs2.Stream
import skunk.Codec
import skunk.Session
import skunk.data.Identifier

import java.time.OffsetDateTime
import java.time.ZoneOffset
import scala.concurrent.duration.*

private final class SkunkOutboxReader[F[_]: Concurrent: Clock, N](
    pool: Resource[F, Session[F]],
    q: Queries.Outbox[N]
) extends OutboxReader[F, N] {
  def read: Stream[F, OutboxItem[N]] = Stream
    .resource(pool.flatMap(_.prepare(q.read)))
    .flatMap(_.stream(skunk.Void, 100))

  def markAllAsSent(items: NonEmptyChain[OutboxItem[N]]): F[Unit] = for {
    now <- currentTime[F]
    is = items.toList.map(_.seqNr)
    _ <- pool
      .flatMap(_.prepare(q.markAsPublished(is)))
      .use(_.execute((now, is)))
  } yield ()
}
