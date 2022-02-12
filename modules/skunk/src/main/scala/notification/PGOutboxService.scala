package edfsm.backend.skunk

import cats.Functor
import cats.MonadError
import cats.effect.*
import cats.effect.implicits.*
import cats.effect.std.Queue
import cats.implicits.*
import edfsm.eventsourcing.*
import fs2.Stream
import fs2.Stream.*
import io.odin.Logger
import io.odin.loggers.ConstContextLogger
import edfsm.backend.skunk.assertInserted
import skunk.*
import skunk.circe.codec.all.jsonb
import skunk.codec.all.*
import skunk.data.Identifier
import skunk.implicits.*

import java.time.OffsetDateTime
import scala.concurrent.duration.*

object PGOutboxService {

  def setup[F[_]: Functor, T: io.circe.Encoder: io.circe.Decoder](
      session: Session[F],
      namespace: String
  ): F[Unit] =
    val queries = Queries(namespace, jsonb[T])
    session.execute(queries.createTable).void

  def apply[F[_], T: io.circe.Encoder: io.circe.Decoder](
      session: Session[F],
      namespace: String,
      logger: Logger[F]
  )(using F: Temporal[F]): F[OutboxService[F, T]] = {
    val ctx = Map("namespace" -> namespace)
    val log = ConstContextLogger(ctx, logger)

    val queries = Queries(namespace, jsonb[T])

    PGOutboxNotification
      .getChannel(session, namespace)
      .map(channel =>
        new OutboxService {
          def outbox(t: T, time: OffsetDateTime): F[Unit] =
            session
              .prepare(queries.insert)
              .use(
                _.execute((t, time)).assertInserted >> channel.notify(namespace)
              )
        }
      )
  }
}
