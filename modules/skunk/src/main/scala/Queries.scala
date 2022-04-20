package edomata.backend

import cats.data.NonEmptyList
import edomata.core.CommandMessage
import edomata.core.MessageMetadata
import skunk.*
import skunk.codec.all.*
import skunk.implicits.*

import java.time.Instant
import java.time.OffsetDateTime
import java.time.ZoneOffset
import java.util.UUID

private[backend] object Queries {
  def setupSchema(namespace: PGNamespace): Command[Void] =
    sql"""create schema if not exists #$namespace;""".command

  final class Journal[E](namespace: PGNamespace, codec: BackendCodec[E]) {
    private val table = sql"#$namespace.journal"
    private val event = codec.codec

    def setup: Command[Void] = sql"""
DO $$$$ begin

CREATE TABLE IF NOT EXISTS $table (
  id uuid NOT NULL,
  "time" timestamptz NOT NULL,
  seqnr bigserial NOT NULL,
  "version" int8 NOT NULL,
  stream text NOT NULL,
  payload #${codec.oid.name} NOT NULL,
  CONSTRAINT journal_pk PRIMARY KEY (id),
  CONSTRAINT journal_un UNIQUE (stream, version)
);

CREATE INDEX IF NOT EXISTS journal_seqnr_idx ON $table USING btree (seqnr);

CREATE INDEX IF NOT EXISTS journal_stream_idx ON $table USING btree (stream, version);

END $$$$;
""".command

    final case class InsertRow(
        id: UUID,
        streamId: String,
        time: OffsetDateTime,
        version: SeqNr,
        event: E
    )

    private val insertRow: Codec[InsertRow] =
      (uuid *: text *: timestamptz *: int8 *: event).pimap[InsertRow]

    def append(
        n: List[InsertRow]
    ): Command[n.type] =
      sql"""insert into $table ("id", "stream", "time", "version", "payload") values ${insertRow.values
          .list(n)}""".command

    private val readFields = sql"id, time, seqnr, version, stream, payload"
    private val metaCodec: Codec[EventMetadata] =
      (uuid *: timestamptz *: int8 *: int8 *: text).pimap
    private val readCodec: Codec[EventMessage[E]] =
      (metaCodec *: event).pimap

    def readAll: Query[Void, EventMessage[E]] =
      sql"select $readFields from $table order by seqnr asc".query(readCodec)

    def readAllAfter: Query[Long, EventMessage[E]] =
      sql"select $readFields from $table where seqnr > $int8 order by seqnr asc"
        .query(readCodec)

    def readStream: Query[String, EventMessage[E]] =
      sql"select $readFields from $table where stream = $text order by version asc"
        .query(readCodec)

    def readStreamAfter: Query[(String, Long), EventMessage[E]] =
      sql"select $readFields from $table where stream = $text and version > $int8 order by version asc"
        .query(readCodec)

    def readStreamBefore: Query[(String, Long), EventMessage[E]] =
      sql"select $readFields from $table where stream = $text and version < $int8 order by version asc"
        .query(readCodec)
  }

  final class Outbox[N](namespace: PGNamespace, codec: BackendCodec[N]) {
    private val table = sql"#$namespace.outbox"
    private val notification = codec.codec

    val setup = sql"""
CREATE TABLE IF NOT EXISTS $table(
  seqnr bigserial NOT NULL,
  correlation text NULL,
  causation text NULL,
  payload #${codec.oid.name} NOT NULL,
  created timestamptz NOT NULL,
  published timestamptz NULL,
  CONSTRAINT outbox_pk PRIMARY KEY (seqnr)
);
""".command

    def markAsPublished(l: List[Long]): Command[(OffsetDateTime, l.type)] =
      sql"""
update $table
set published = $timestamptz
where seqnr in ${int8.values.list(l)}
""".command

    private val metadata: Codec[MessageMetadata] = (text.opt *: text.opt).pimap
    private val itemCodec: Codec[OutboxItem[N]] =
      (int8 *: timestamptz *: notification *: metadata).pimap

    val read: Query[Void, OutboxItem[N]] =
      sql"""
select seqnr, created, payload, correlation, causation
from $table
where published is NULL
order by seqnr asc
limit 10
""".query(itemCodec)

    type BatchInsert = List[(N, OffsetDateTime, MessageMetadata)]

    private val insertCodec = (notification *: timestamptz *: metadata)
    def insertAll(items: BatchInsert): Command[items.type] =
      sql"""
insert into $table (payload, created, correlation, causation) values ${insertCodec.values
          .list(items)}
""".command
  }

  final class Snapshot[S, E, R](
      namespace: PGNamespace,
      codec: BackendCodec[S]
  ) {
    private val table = sql"#$namespace.snapshots"
    private val state = codec.codec

    val setup: Command[Void] = sql"""
CREATE TABLE IF NOT EXISTS $table (
  id text NOT NULL,
  "version" int8 NOT NULL,
  state #${codec.oid.name} NOT NULL,
  CONSTRAINT snapshots_pk PRIMARY KEY (id)
);
""".command

    private def aggregateStateCodec: Codec[AggregateState.Valid[S]] =
      (state *: int8).pimap

    def put: Command[(String, AggregateState.Valid[S])] =
      sql"""insert into $table (id, "version", state) values ($text, $aggregateStateCodec)
            on conflict (id) do update
                                set version = excluded.version,
                                    state   = excluded.state
         """.command

    def get: Query[String, AggregateState.Valid[S]] =
      sql"""select version, state from $table where id = $text""".query(
        aggregateStateCodec
      )
  }

  final class Commands(namespace: PGNamespace) {
    private val table = sql"#$namespace.commands"

    val setup: Command[Void] = sql"""
CREATE TABLE IF NOT EXISTS $table (
  id text NOT NULL,
  "time" timestamptz NOT NULL,
  address text NOT NULL,
  CONSTRAINT commands_pk PRIMARY KEY (id)
);
""".command

    val count: Query[String, Long] =
      sql"select count(*) from $table where id = $text".query(int8)

    private val instant: Codec[Instant] =
      timestamptz.imap(_.toInstant)(_.atOffset(ZoneOffset.UTC))

    private val command: Encoder[CommandMessage[?]] =
      (text *: text *: instant).contramap(c => (c.id, c.address, c.time))

    def insert: Command[CommandMessage[?]] = sql"""
insert into $table (id, address, "time") values ($command);
""".command
  }
}
