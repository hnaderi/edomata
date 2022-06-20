/*
 * Copyright 2021 Hossein Naderi
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

package edomata.backend
package doobie

import _root_.doobie.*
import _root_.doobie.free.connection.ConnectionIO
import _root_.doobie.implicits.*
import _root_.doobie.postgres.implicits.*
import _root_.doobie.util.fragment.Fragment
import cats.data.NonEmptyChain
import edomata.core.CommandMessage
import edomata.core.MessageMetadata

import java.time.OffsetDateTime
import java.util.UUID

private[backend] object Queries {
  private def escapeStr(name: PGNamespace) = s""""$name""""
  private def escape(name: PGNamespace) = Fragment.const(escapeStr(name))

  def setupSchema(namespace: PGNamespace): Update0 =
    sql"""create schema if not exists ${escape(namespace)};""".update

  final class Journal[E](namespace: PGNamespace, codec: BackendCodec[E]) {
    private val table = sql"${escape(namespace)}.journal"
    private val tableStr = s"${escapeStr(namespace)}.journal"
    private val payloadOid = Fragment.const(codec.tpe)
    private given Meta[E] = codec.codec

    def setup = sql"""
DO $$$$ begin

CREATE TABLE IF NOT EXISTS $table (
  id uuid NOT NULL,
  "time" timestamptz NOT NULL,
  seqnr bigserial NOT NULL,
  "version" int8 NOT NULL,
  stream text NOT NULL,
  payload $payloadOid NOT NULL,
  CONSTRAINT journal_pk PRIMARY KEY (id),
  CONSTRAINT journal_un UNIQUE (stream, version)
);

CREATE INDEX IF NOT EXISTS journal_seqnr_idx ON $table USING btree (seqnr);

CREATE INDEX IF NOT EXISTS journal_stream_idx ON $table USING btree (stream, version);

END $$$$;
""".update

    final case class InsertRow(
        id: UUID,
        streamId: String,
        time: OffsetDateTime,
        version: SeqNr,
        event: E
    )

    def append(
        n: List[InsertRow]
    ): ConnectionIO[Int] =
      val sql =
        s"""insert into $tableStr ("id", "stream", "time", "version", "payload") values (?, ?, ?, ?, ?)"""
      Update[InsertRow](sql).updateMany(n)

    private val readFields = sql"id, time, seqnr, version, stream, payload"

    def readAll: Query0[EventMessage[E]] =
      sql"select $readFields from $table order by seqnr asc".query

    def readAllAfter(seqNr: Long): Query0[EventMessage[E]] =
      sql"select $readFields from $table where seqnr > $seqNr order by seqnr asc".query

    def readAllBefore(seqNr: Long): Query0[EventMessage[E]] =
      sql"select $readFields from $table where seqnr < $seqNr order by seqnr asc".query

    def readStream(stream: String): Query0[EventMessage[E]] =
      sql"select $readFields from $table where stream = $stream order by version asc".query

    def readStreamAfter(
        stream: String,
        version: Long
    ): Query0[EventMessage[E]] =
      sql"select $readFields from $table where stream = $stream and version > $version order by version asc".query

    def readStreamBefore(
        stream: String,
        version: Long
    ): Query0[EventMessage[E]] =
      sql"select $readFields from $table where stream = $stream and version < $version order by version asc".query
  }

  final class Outbox[N](namespace: PGNamespace, codec: BackendCodec[N]) {
    private val table = sql"${escape(namespace)}.outbox"
    private val tableStr = s"${escapeStr(namespace)}.outbox"
    private given Meta[N] = codec.codec

    val setup: Update0 = sql"""
CREATE TABLE IF NOT EXISTS $table(
  seqnr bigserial NOT NULL,
  stream text NOT NULL,
  correlation text NULL,
  causation text NULL,
  payload ${Fragment.const(codec.tpe)} NOT NULL,
  created timestamptz NOT NULL,
  published timestamptz NULL,
  CONSTRAINT outbox_pk PRIMARY KEY (seqnr)
);
""".update

    def markAsPublished(l: NonEmptyChain[Long], time: OffsetDateTime): Update0 =
      sql"""
update $table
set published = $time
where ${Fragments.in(fr"seqnr", l)}
""".update

    val read: Query0[OutboxItem[N]] =
      sql"""
select seqnr, stream, created, payload, correlation, causation
from $table
where published is NULL
order by seqnr asc
""".query

    type ToInsert = (N, String, OffsetDateTime, MessageMetadata)
    def insertAll(items: List[ToInsert]): ConnectionIO[Int] =
      val sql = s"""
insert into $tableStr (payload, stream, created, correlation, causation) values (?, ?, ?, ?, ?)
"""
      Update[ToInsert](sql).updateMany(items)
  }

  final class Snapshot[S](
      namespace: PGNamespace,
      codec: BackendCodec[S]
  ) {
    private val table = sql"${escape(namespace)}.snapshots"
    private val tableStr = s"${escapeStr(namespace)}.snapshots"
    private given Meta[S] = codec.codec

    val setup: Update0 = sql"""
CREATE TABLE IF NOT EXISTS $table (
  id text NOT NULL,
  "version" int8 NOT NULL,
  state ${Fragment.const(codec.tpe)} NOT NULL,
  CONSTRAINT snapshots_pk PRIMARY KEY (id)
);
""".update

    type ToInsert = (String, AggregateState.Valid[S])
    def put(l: List[ToInsert]): ConnectionIO[Int] =
      val sql = s"""
insert into $tableStr (id, state, "version") values (?, ?, ?)
on conflict (id) do update
set version = excluded.version,
    state   = excluded.state
         """
      Update[ToInsert](sql).updateMany(l)

    def get(id: String): Query0[AggregateState.Valid[S]] =
      sql"""select state , version from $table where id = $id""".query
  }

  final class Commands(namespace: PGNamespace) {
    private val table = sql"${escape(namespace)}.commands"

    val setup: Update0 = sql"""
CREATE TABLE IF NOT EXISTS $table (
  id text NOT NULL,
  "time" timestamptz NOT NULL,
  address text NOT NULL,
  CONSTRAINT commands_pk PRIMARY KEY (id)
);
""".update

    def count(id: String): Query0[Long] =
      sql"select count(*) from $table where id = $id".query

    def insert(cmd: CommandMessage[?]): Update0 = sql"""
insert into $table (id, address, "time") values (${cmd.id}, ${cmd.address}, ${cmd.time})
""".update
  }
}
