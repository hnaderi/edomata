package edfsm.backend

import cats.implicits.*
import io.circe.syntax.*
import edfsm.backend.CommandMessage
import skunk.*
import skunk.circe.codec.all.jsonb
import skunk.codec.all.*
import skunk.implicits.*
import java.time.Instant
import java.time.ZoneOffset
import cats.Functor
import cats.effect.kernel.MonadCancel

object PGCommandStore {
  object Queries {
    val createTable: Command[Void] = sql"""
CREATE TABLE IF NOT EXISTS commands (
  id text NOT NULL,
  "time" timestamptz NOT NULL,
  address text NOT NULL,
  payload jsonb NOT NULL,
  CONSTRAINT commands_pk PRIMARY KEY (id)
);
""".command

    val count: Query[String, Long] =
      sql"select count(*) from commands where id = $text".query(int8)

    val instant: Codec[Instant] =
      timestamptz.imap(_.toInstant)(_.atOffset(ZoneOffset.UTC))
    def insert[T](using payload: Encoder[T]): Command[CommandMessage[T]] = {
      val row: Encoder[CommandMessage[T]] =
        (text *: instant *: text *: payload).pcontramap

      sql"""insert into commands (id, "time", address, payload) values ${row.values} """.command
    }
  }

  def setup[F[_]: Functor](session: Session[F]): F[Unit] =
    session.execute(Queries.createTable).void

  def apply[F[_], T: io.circe.Encoder](
      session: Session[F]
  )(using MonadCancel[F, Throwable]): CommandStore[F, T] =
    given Encoder[T] = jsonb.contramap(t => t.asJson)
    new CommandStore {
      def append(cmd: CommandMessage[T]): F[Unit] =
        session.prepare(Queries.insert[T]).use(_.execute(cmd).void)
      def contains(id: String): F[Boolean] =
        session.prepare(Queries.count).use(_.unique(id).map(_ == 1))
    }
}
