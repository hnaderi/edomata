package edfsm.common.notification

import cats.MonadError
import cats.effect.Concurrent
import cats.effect.Resource
import cats.effect.std.Queue
import cats.implicits.*
import fs2.Pipe
import fs2.Stream
import fs2.Stream.*
import io.circe.Decoder
import io.circe.Encoder
import io.odin.Logger
import io.odin.loggers.ConstContextLogger
import edfsm.common.*
import skunk.Channel
import skunk.Session
import skunk.circe.codec.json
import skunk.data.Identifier

import java.time.OffsetDateTime
import java.time.ZoneOffset

import PGOutboxNotification.*

/** Builder for notifications for outboxed messages using a single long lived
  * session for listening
  */
final class PGOutboxNotification[F[_]: Concurrent](
    session: Session[F],
    logger: Logger[F]
) {

  /** Bufferred listener for outboxed messages in a specific namespace
    *
    * NOTE that at most one notification is maintained at a time and extra
    * notifications are dropped.
    */
  def stream(namespace: String): Stream[F, Unit] =
    val ctx = Map("namespace" -> namespace)
    eval(getChannel(session, namespace)).flatMap { channel =>
      emit(()) ++
        channel
          .listen(1)
          .through(keepLatest)
          .evalMap(i =>
            logger.trace(s"PG LISTEN on $namespace ${i.toString}", ctx)
          )
          .onFinalizeCase(e =>
            logger.debug(
              s"Outbox consume for $namespace terminated , cause: $e",
              ctx
            )
          )
    }
}

object PGOutboxNotification {

  /** Creates an OutboxNotification that takes a single long lived session from
    * pool
    */
  def apply[F[_]: Concurrent](
      pool: Resource[F, Session[F]],
      logger: Logger[F]
  ): Resource[F, PGOutboxNotification[F]] =
    pool.map(new PGOutboxNotification(_, logger))

  private def keepLatest[F[_]: Concurrent, T]: fs2.Pipe[F, T, T] =
    in =>
      eval(Queue.circularBuffer[F, T](1)).flatMap { q =>
        in.enqueueUnterminated(q) mergeHaltL Stream.fromQueueUnterminated(q, 1)
      }

  private[notification] def getChannel[F[_]](
      session: Session[F],
      namespace: String
  )(using
      F: MonadError[F, Throwable]
  ): F[Channel[F, String, String]] =
    F.fromEither(
      Identifier
        .fromString(s"${namespace}_outbox")
        .leftMap(BadNamespace(namespace, _))
    ).map(session.channel)

  final case class BadNamespace(name: String, msg: String)
      extends Exception(s"name: $name msg: $msg")
}
