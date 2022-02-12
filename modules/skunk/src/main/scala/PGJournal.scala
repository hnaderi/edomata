package edfsm.eventsourcing

import cats.FlatMap
import cats.Monad
import cats.MonadError
import cats.data.NonEmptyChain
import cats.effect.Concurrent
import cats.effect.*
import cats.effect.implicits.*
import cats.implicits.*
import edfsm.eventsourcing.ESRepository
import edfsm.eventsourcing.Journal
import edfsm.eventsourcing.MyESRepository
import edfsm.eventsourcing.*
import fs2.Stream
import io.odin.Logger
import edfsm.common.pgPersistence.assertInsertedSize
import skunk.*
import skunk.circe.codec.all.jsonb
import skunk.codec.all.*
import skunk.data.Completion
import skunk.implicits.*

import java.time.OffsetDateTime
import java.time.ZoneOffset
import java.util.UUID

object PGJournal {
  final case class InsertRow[E](
      id: UUID,
      streamId: String,
      time: OffsetDateTime,
      version: SeqNr,
      event: E
  )
  final case class Queries(namespace: String) {
    private val escName = s""""$namespace""""
    val table = sql"#$escName.journal"

    val createTable: Command[Void] = sql"""
CREATE TABLE IF NOT EXISTS $table (
  id uuid NOT NULL,
  "time" timestamptz NOT NULL,
  seqnr bigserial NOT NULL,
  "version" int8 NOT NULL,
  stream text NOT NULL,
  payload jsonb NOT NULL,
  CONSTRAINT journal_pk PRIMARY KEY (id),
  CONSTRAINT journal_un UNIQUE (stream, version)
);
""".command
    val createSeqNrIndex: Command[Void] = sql"""
CREATE INDEX IF NOT EXISTS journal_seqnr_idx ON $table USING btree (seqnr);
""".command
    val createStreamIndex: Command[Void] = sql"""
CREATE INDEX IF NOT EXISTS journal_stream_idx ON $table USING btree (stream, version);
""".command

    def append[E](
        n: List[InsertRow[E]]
    )(using evCodec: Codec[E]): Command[n.type] = {
      val codec: Codec[InsertRow[E]] =
        (uuid *: text *: timestamptz *: int8 *: evCodec).pimap[InsertRow[E]]

      sql"""insert into $table ("id", "stream", "time", "version", "payload") values ${codec.values
        .list(n)}""".command
    }

    val readFields = sql"id, time, seqnr, version, stream, payload"
    val metaCodec: Codec[EventMetadata] =
      (uuid *: timestamptz *: int8 *: int8 *: text).pimap
    def readCodec[E](using codec: Codec[E]): Codec[EventMessage[E]] =
      (metaCodec *: codec).pimap

    def readAll[E](using
        codec: Codec[EventMessage[E]]
    ): Query[Void, EventMessage[E]] =
      sql"select $readFields from $table order by seqnr asc".query(codec)

    def readAllAfter[E](using
        codec: Codec[EventMessage[E]]
    ): Query[Long, EventMessage[E]] =
      sql"select $readFields from $table where seqnr > $int8 order by seqnr asc"
        .query(codec)

    def readStream[E](using
        codec: Codec[EventMessage[E]]
    ): Query[String, EventMessage[E]] =
      sql"select $readFields from $table where stream = $text order by version asc"
        .query(codec)

    def readStreamAfter[E](using
        codec: Codec[EventMessage[E]]
    ): Query[(String, Long), EventMessage[E]] =
      sql"select $readFields from $table where stream = $text and version > $int8 order by version asc"
        .query(codec)
  }

  def setup[F[_]: FlatMap](
      namespace: String,
      session: Session[F]
  ): F[Unit] =
    val queries = Queries(namespace)
    session.execute(queries.createTable) >>
      session.execute(queries.createSeqNrIndex) >>
      session.execute(queries.createStreamIndex).void

  def apply[F[_], E: io.circe.Encoder: io.circe.Decoder](
      namespace: String,
      session: Session[F]
  )(using F: Sync[F]): Journal[F, String, E] =
    val queries = Queries(namespace)
    given Codec[E] = jsonb[E]
    given Codec[EventMessage[E]] = queries.readCodec[E]
    new Journal {
      private val readAllQ = session.prepare(queries.readAll)
      private val readAllAfterQ = session.prepare(queries.readAllAfter)
      private val readStreamQ = session.prepare(queries.readStream)
      private val readStreamAfterQ = session.prepare(queries.readStreamAfter)
      private val channel = session.channel(id"journal")

      def append(
          streamId: String,
          time: OffsetDateTime,
          version: SeqNr,
          events: NonEmptyChain[E]
      ): F[Unit] = {
        val evList = events.toList
        for {
          ids <- evList.traverse(_ => F.delay(UUID.randomUUID))
          list = ids.zip(evList).zipWithIndex.map { case ((id, ev), idx) =>
            InsertRow(
              id = id,
              streamId = streamId,
              time = time,
              version = version + idx + 1,
              event = ev
            )
          }
          _ <- session
            .prepare(queries.append(list))
            .use(_.execute(list))
            .assertInsertedSize(list.size)
          _ <- channel.notify(streamId)
        } yield ()
      }
      def readStream(streamId: String): Stream[F, EventMessage[E]] =
        Stream.resource(readStreamQ).flatMap(_.stream(streamId, 100))
      def readStreamAfter(
          streamId: String,
          version: EventVersion
      ): Stream[F, EventMessage[E]] =
        Stream
          .resource(readStreamAfterQ)
          .flatMap(_.stream((streamId, version), 100))
      def readAll: Stream[F, EventMessage[E]] =
        Stream.resource(readAllQ).flatMap(_.stream(Void, 100))
      def readAllAfter(seqNr: SeqNr): Stream[F, EventMessage[E]] =
        Stream.resource(readAllAfterQ).flatMap(_.stream(seqNr, 100))
      def notifications: Stream[F, String] = channel.listen(1000).map(_.value)
    }
}
