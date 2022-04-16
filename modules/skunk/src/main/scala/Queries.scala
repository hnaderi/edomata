package edomata.backend

import edomata.core.CommandMessage
import edomata.core.MessageMetadata
import skunk.*
import skunk.codec.all.*
import skunk.implicits.*

import java.time.Instant
import java.time.OffsetDateTime
import java.time.ZoneOffset

private[backend] object Queries {
  final class Journal[E](namespace: String, codec: BackendCodec[E]) {
    private val table = sql"#$namespace.journal"
    private val event = codec.codec

    def setup: Command[Void] = sql"""
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
""".command

    val createSeqNrIndex: Command[Void] = sql"""
CREATE INDEX IF NOT EXISTS journal_seqnr_idx ON $table USING btree (seqnr);
""".command
    val createStreamIndex: Command[Void] = sql"""
CREATE INDEX IF NOT EXISTS journal_stream_idx ON $table USING btree (stream, version);
""".command

    // def append(
    //     n: List[InsertRow[E]]
    // )(using evCodec: Codec[E]): Command[n.type] = {
    //   val codec: Codec[InsertRow[E]] =
    //     (uuid *: text *: timestamptz *: int8 *: evCodec).pimap[InsertRow[E]]

    //   sql"""insert into $table ("id", "stream", "time", "version", "payload") values ${codec.values
    //       .list(n)}""".command
    // }

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

  final class Outbox[N](namespace: String, codec: BackendCodec[N]) {
    private val table = sql"#$namespace.outbox"
    private val notification = codec.codec

    val setup = sql"""
CREATE TABLE IF NOT EXISTS $table(
  seqnr bigserial NOT NULL,
  payload #${codec.oid.name} NOT NULL,
  created timestamptz NOT NULL,
  published timestamptz NULL,
  CONSTRAINT outbox_pk PRIMARY KEY (seqnr)
);
""".command

    val publish: Command[(OffsetDateTime, Long)] = sql"""
update $table
set published = $timestamptz
where seqnr <= $int8
  and published is NULL
""".command

    val markAsPublished: Command[(OffsetDateTime, Long)] = sql"""
update $table
set published = $timestamptz
where seqnr = $int8
""".command

    private val metadata: Codec[MessageMetadata] = ???
    private val itemCodec: Codec[OutboxItem[N]] =
      (int8 *: timestamptz *: notification *: metadata).pimap

    val read: Query[Void, OutboxItem[N]] =
      sql"""
select seqnr, created, payload
from $table
where published is NULL
order by seqnr asc
limit 10
""".query(itemCodec)

    val insert: Command[(N, OffsetDateTime)] = sql"""
insert into $table (payload, created) values ($notification, $timestamptz)
""".command
  }

  final class Snapshot[S, E, R](namespace: String, codec: BackendCodec[S]) {
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

    private def aggregateStateCodec: Codec[AggregateState.Valid[S, E, R]] =
      (state *: int8).pimap

    def put: Command[(String, AggregateState.Valid[S, E, R])] =
      sql"""insert into $table (id, "version", state) values ($text, $aggregateStateCodec)
            on conflict (id) do update
                                set version = excluded.version,
                                    state   = excluded.state
         """.command

    def get: Query[String, AggregateState.Valid[S, E, R]] =
      sql"""select version, state from $table where id = $text""".query(
        aggregateStateCodec
      )
  }

  final class Commands(namespace: String) {
    private val table = sql"#$namespace.commands"

    val setup: Command[Void] = sql"""
CREATE TABLE IF NOT EXISTS commands (
  id text NOT NULL,
  "time" timestamptz NOT NULL,
  address text NOT NULL,
  CONSTRAINT commands_pk PRIMARY KEY (id)
);
""".command

    val count: Query[String, Long] =
      sql"select count(*) from commands where id = $text".query(int8)

    private val instant: Codec[Instant] =
      timestamptz.imap(_.toInstant)(_.atOffset(ZoneOffset.UTC))

    def insert: Command[CommandMessage[?]] = ???
  }
}
