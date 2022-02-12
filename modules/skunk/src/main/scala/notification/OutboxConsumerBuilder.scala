package edfsm.backend.skunk

import cats.effect.Resource
import cats.effect.Temporal
import cats.implicits.*
import fs2.Stream
import io.circe.Decoder
import io.circe.Encoder
import io.odin.Logger

import skunk.Session

import PGOutboxConsumer.*

trait OutboxConsumerBuilder[F[_]] {
  def build[T: Encoder: Decoder](
      namespace: String,
      func: OutboxItem[T] => F[Unit]
  ): Stream[F, Unit]
}

object OutboxConsumerBuilder {
  def apply[F[_]: Temporal](
      pool: Resource[F, Session[F]],
      logger: Logger[F]
  ): Resource[F, OutboxConsumerBuilder[F]] =
    val consumer = PGOutboxConsumer(pool, logger)
    PGOutboxNotification(pool, logger).map(outn =>
      new OutboxConsumerBuilder[F] {
        def build[T: Encoder: Decoder](
            namespace: String,
            func: OutboxItem[T] => F[Unit]
        ): Stream[F, Unit] = consumer.build(namespace, outn, func)
      }
    )
}
