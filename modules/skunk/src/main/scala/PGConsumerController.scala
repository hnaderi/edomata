package edfsm.eventsourcing

import cats.Monad
import cats.MonadError
import cats.data.NonEmptyChain
import cats.effect.Concurrent
import cats.effect.*
import cats.effect.implicits.*
import cats.implicits.*
import edfsm.core.Request
import edfsm.eventsourcing.ESRepository
import edfsm.eventsourcing.Journal
import edfsm.eventsourcing.MyESRepository
import edfsm.eventsourcing.*
import fs2.Stream
import io.odin.Logger
import edfsm.common.pgPersistence.*
import skunk.*
import skunk.circe.codec.all.jsonb
import skunk.codec.all.*
import skunk.data.Completion
import skunk.implicits.*

import java.time.OffsetDateTime

object PGConsumerController {
  object Queries {
    val createTable: Command[Void] = sql"""
CREATE TABLE IF NOT EXISTS consumers (
  name text NOT NULL,
  "seqnr" int8 NOT NULL,
  CONSTRAINT consumers_pk PRIMARY KEY (name)
);
""".command

    val upsert: Command[(ConsumerName, SeqNr)] = sql"""
insert into consumers (name, seqnr) values ($text, $int8)
on conflict do update set seqnr = excluded.seqnr
""".command

    val select: Query[ConsumerName, SeqNr] =
      sql"select seqnr from consumers where name = $text".query(int8)

  }

  def apply[F[_]](
      session: Session[F]
  )(using
      MonadError[F, Throwable]
  ): Resource[F, ConsumerController[F]] = for {
    _ <- Resource.eval(session.execute(Queries.createTable))
    upsert <- session.prepare(Queries.upsert)
    select <- session.prepare(Queries.select)
  } yield new ConsumerController[F] {
    def seek(name: ConsumerName, seqNr: SeqNr): F[Unit] =
      upsert.execute((name, seqNr)).void
    def read(name: ConsumerName): F[Option[SeqNr]] = select.option(name)
  }
}
