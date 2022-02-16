package edomata.backend.skunk

import cats.effect.Concurrent
import cats.effect.Resource
import cats.effect.Temporal
import cats.effect.kernel.Ref
import cats.effect.std.Semaphore
import cats.implicits.*
import fs2.Pipe
import fs2.Stream
import fs2.Stream.*
import io.circe.Decoder
import io.circe.Encoder
import io.odin.Logger
import io.odin.loggers.ConstContextLogger

import skunk.Session
import skunk.circe.codec.json

import java.time.OffsetDateTime
import java.time.ZoneOffset

import PGOutboxConsumer.*

final case class PGOutboxConsumer[F[_]: Temporal](
    pool: Resource[F, Session[F]],
    logger: Logger[F]
) {
  def build[T: Encoder: Decoder](
      namespace: String,
      notification: PGOutboxNotification[F],
      func: OutboxItem[T] => F[Unit]
  ): Stream[F, Unit] =
    notification.stream(namespace).through(pipe(namespace, func))

  private[skunk] def pipe[T: Encoder: Decoder](
      namespace: String,
      func: OutboxItem[T] => F[Unit]
  ): Pipe[F, Unit, Unit] = {
    val ctx = Map("namespace" -> namespace)
    val log = ConstContextLogger(ctx, logger)
    val queries = Statements[F, T](pool, namespace)

    def handle(
        q: Statements[F, T]
    )(last: Option[Long]): F[(Boolean, Option[Long])] = for {
      _ <- log.debug(s"Continuing from $last")
      page <- q.getPage(last)
      _ <- log.trace(s"Notif page size ${page.size}")
      now <- Temporal[F].realTimeInstant.map(_.atOffset(ZoneOffset.UTC))
      _ <- page.traverse(item =>
        func(item) >> q.markAsPublished(item.seqNr, now)
      )
      nextOffset = page.lastOption.map(_.seqNr)
    } yield (page.size == 10, nextOffset)

    def continueFrom(last: Option[Long]): Stream[F, Long] =
      resource(Statements[F, T](pool, namespace))
        .evalMap(handle(_)(last))
        .flatMap((cont, l) => if cont then continueFrom(l) else emits(l.toSeq))

    _.evalScan(Option.empty[Long])((last, _) =>
      continueFrom(last).compile.last
    ).void.onFinalize(log.info("Terminated!"))
  }
}

object PGOutboxConsumer {
  private final case class Statements[F[_], T](
      getPage: Option[Long] => F[List[OutboxItem[T]]],
      markAsPublished: (Long, OffsetDateTime) => F[Unit]
  )
  private object Statements {

    def apply[F[_]: Concurrent, T: Encoder: Decoder](
        session: Session[F],
        namespace: String
    ): Resource[F, Statements[F, T]] = {
      val rawQueries = Queries(namespace, json.jsonb[T])
      val markQuery =
        session
          .prepare(rawQueries.markAsPublished)
          .map(pq =>
            (until: Long, time: OffsetDateTime) =>
              pq.execute((time, until)).void
          )
      val readQuery =
        session
          .prepare(rawQueries.read)
          .map(pq =>
            (offset: Option[Long]) => pq.stream(skunk.Void, 11).compile.toList
          )

      (readQuery, markQuery).mapN(Statements(_, _))
    }

    def apply[F[_]: Concurrent, T: Encoder: Decoder](
        pool: Resource[F, Session[F]],
        namespace: String
    ): Resource[F, Statements[F, T]] = pool.flatMap(apply(_, namespace))
  }
}
