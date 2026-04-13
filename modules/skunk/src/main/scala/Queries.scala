/*
 * Copyright 2021 Beyond Scale Group
 *
 * Licensed under the Apache License, Version 2.0 (the "License");
 * you may not use this file except in compliance with the License.
 * You may obtain a copy of the License at
 *
 *     http://www.apache.org/licenses/LICENSE-2.0
 *
 * Unless required by applicable law or agreed to in writing, software
 * distributed under the License is distributed on an "AS IS" BASIS,
 * WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
 * See the License for the specific language governing permissions and
 * limitations under the License.
 */

package edomata.skunk

import _root_.skunk.*
import _root_.skunk.codec.all.*
import _root_.skunk.implicits.*
import edomata.backend.*
import edomata.backend.eventsourcing.AggregateState
import edomata.core.CommandMessage
import edomata.core.MessageMetadata

import java.time.Instant
import java.time.OffsetDateTime
import java.time.ZoneOffset
import java.util.UUID

private[skunk] object Queries {
  def setupSchema(ns: PGNamespace): Command[Void] =
    sql"""create schema if not exists "#$ns";""".command

  final class Journal[E](naming: PGNaming, codec: BackendCodec[E]) {
    private val t = naming.table("journal")
    private val table = sql"""#$t"""
    private val event = codec.codec
    private val pk = naming.constraint("journal_pk")
    private val un = naming.constraint("journal_un")
    private val seqnrIdx = naming.index("journal_seqnr_idx")
    private val streamIdx = naming.index("journal_stream_idx")

    def setup: Command[Void] = sql"""
DO $$$$ begin

CREATE TABLE IF NOT EXISTS #$t (
  id uuid NOT NULL,
  "time" timestamptz NOT NULL,
  seqnr bigserial NOT NULL,
  "version" int8 NOT NULL,
  stream text NOT NULL,
  payload #${codec.oid.name} NOT NULL,
  CONSTRAINT #$pk PRIMARY KEY (id),
  CONSTRAINT #$un UNIQUE (stream, version)
);

CREATE INDEX IF NOT EXISTS #$seqnrIdx ON #$t USING btree (seqnr);

CREATE INDEX IF NOT EXISTS #$streamIdx ON #$t USING btree (stream, version);

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
      (uuid *: text *: timestamptz *: int8 *: event).to[InsertRow]

    def append(
        n: List[InsertRow]
    ): Command[n.type] =
      sql"""insert into $table ("id", "stream", "time", "version", "payload") values ${insertRow.values
          .list(n)}""".command

    private val readFields = sql"id, time, seqnr, version, stream, payload"
    private val metaCodec: Codec[EventMetadata] =
      (uuid *: timestamptz *: int8 *: int8 *: text).to
    private val readCodec: Codec[EventMessage[E]] =
      (metaCodec *: event).to

    def readAll: Query[Void, EventMessage[E]] =
      sql"select $readFields from $table order by seqnr asc".query(readCodec)

    def readAllAfter: Query[Long, EventMessage[E]] =
      sql"select $readFields from $table where seqnr > $int8 order by seqnr asc"
        .query(readCodec)

    def readAllBefore: Query[Long, EventMessage[E]] =
      sql"select $readFields from $table where seqnr < $int8 order by seqnr asc"
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

  final class Outbox[N](naming: PGNaming, codec: BackendCodec[N]) {
    private val t = naming.table("outbox")
    private val table = sql"""#$t"""
    private val notification = codec.codec
    private val pk = naming.constraint("outbox_pk")

    val setup = sql"""
CREATE TABLE IF NOT EXISTS #$t(
  seqnr bigserial NOT NULL,
  stream text NOT NULL,
  correlation text NULL,
  causation text NULL,
  payload #${codec.oid.name} NOT NULL,
  created timestamptz NOT NULL,
  published timestamptz NULL,
  CONSTRAINT #$pk PRIMARY KEY (seqnr)
);
""".command

    def markAsPublished(l: List[Long]): Command[(OffsetDateTime, l.type)] =
      sql"""
update $table
set published = $timestamptz
where seqnr in ${int8.list(l).values}
""".command

    private val metadata: Codec[MessageMetadata] = (text.opt *: text.opt).to
    private val itemCodec: Codec[OutboxItem[N]] =
      (int8 *: text *: timestamptz *: notification *: metadata).to

    val read: Query[Void, OutboxItem[N]] =
      sql"""
select seqnr, stream, created, payload, correlation, causation
from $table
where published is NULL
order by seqnr asc
""".query(itemCodec)

    type BatchInsert = List[(N, String, OffsetDateTime, MessageMetadata)]

    private val insertCodec = (notification *: text *: timestamptz *: metadata)
    def insertAll(items: BatchInsert): Command[items.type] =
      sql"""
insert into $table (payload, stream, created, correlation, causation) values ${insertCodec.values
          .list(items)}
""".command
  }

  final class Snapshot[S](
      naming: PGNaming,
      codec: BackendCodec[S]
  ) {
    private val t = naming.table("snapshots")
    private val table = sql"""#$t"""
    private val state = codec.codec
    private val pk = naming.constraint("snapshots_pk")

    val setup: Command[Void] = sql"""
CREATE TABLE IF NOT EXISTS #$t (
  id text NOT NULL,
  "version" int8 NOT NULL,
  state #${codec.oid.name} NOT NULL,
  CONSTRAINT #$pk PRIMARY KEY (id)
);
""".command

    private def aggregateStateCodec: Codec[AggregateState.Valid[S]] =
      (state *: int8).to

    private val insertCodec = (text *: aggregateStateCodec).values

    def put(l: List[(String, AggregateState.Valid[S])]): Command[l.type] =
      sql"""
insert into $table (id, state, "version") values ${insertCodec.list(l)}
on conflict (id) do update
set version = excluded.version,
    state   = excluded.state
         """.command

    def get: Query[String, AggregateState.Valid[S]] =
      sql"""select state , version from $table where id = $text""".query(
        aggregateStateCodec
      )
  }

  final class Commands(naming: PGNaming) {
    private val t = naming.table("commands")
    private val table = sql"""#$t"""
    private val pk = naming.constraint("commands_pk")

    val setup: Command[Void] = sql"""
CREATE TABLE IF NOT EXISTS #$t (
  id text NOT NULL,
  "time" timestamptz NOT NULL,
  address text NOT NULL,
  CONSTRAINT #$pk PRIMARY KEY (id)
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

  final class State[S](
      naming: PGNaming,
      codec: BackendCodec[S]
  ) {
    private val t = naming.table("states")
    private val table = sql"""#$t"""
    private val state = codec.codec
    private val pk = naming.constraint("states_pk")

    val setup: Command[Void] = sql"""
CREATE TABLE IF NOT EXISTS #$t (
  id text NOT NULL,
  "version" int8 NOT NULL,
  state #${codec.oid.name} NOT NULL,
  CONSTRAINT #$pk PRIMARY KEY (id)
);
""".command

    import cqrs.AggregateState

    private def aggregateStateCodec: Codec[AggregateState[S]] =
      (state *: int8).to

    def put: Command[String *: S *: SeqNr *: EmptyTuple] =
      sql"""
insert into $table (id, state, "version") values ($text, $state, 1)
on conflict (id) do update
set version = $table.version + 1,
    state   = excluded.state
where $table.version = $int8
         """.command

    def get: Query[String, AggregateState[S]] =
      sql"""select state , version from $table where id = $text""".query(
        aggregateStateCodec
      )
  }
}
