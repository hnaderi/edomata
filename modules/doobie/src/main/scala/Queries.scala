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
import _root_.doobie.implicits.*
import _root_.doobie.util.fragment.Fragment

private[backend] object Queries {
  def setupSchema(namespace: PGNamespace): Update0 =
    sql"""create schema if not exists ${Fragment.const(namespace)};""".update

  final class Journal[E](namespace: PGNamespace, codec: BackendCodec[E]) {
    private val table = Fragment.const(s"$namespace.journal")
    private val payloadOid = Fragment.const(codec.tpe)
    private val event = codec.codec

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

    // final case class InsertRow(
    //     id: UUID,
    //     streamId: String,
    //     time: OffsetDateTime,
    //     version: SeqNr,
    //     event: E
    // )

    // private val insertRow: Codec[InsertRow] =
    //   (uuid *: text *: timestamptz *: int8 *: event).pimap[InsertRow]

    // def append(
    //     n: List[InsertRow]
    // ): Command[n.type] =
    //   sql"""insert into $table ("id", "stream", "time", "version", "payload") values ${insertRow.values
    //       .list(n)}""".command

    // private val readFields = sql"id, time, seqnr, version, stream, payload"
    // private val metaCodec: Codec[EventMetadata] =
    //   (uuid *: timestamptz *: int8 *: int8 *: text).pimap
    // private val readCodec: Codec[EventMessage[E]] =
    //   (metaCodec *: event).pimap

    // def readAll: Query[Void, EventMessage[E]] =
    //   sql"select $readFields from $table order by seqnr asc".query(readCodec)

    // def readAllAfter: Query[Long, EventMessage[E]] =
    //   sql"select $readFields from $table where seqnr > $int8 order by seqnr asc"
    //     .query(readCodec)

    // def readStream: Query[String, EventMessage[E]] =
    //   sql"select $readFields from $table where stream = $text order by version asc"
    //     .query(readCodec)

    // def readStreamAfter: Query[(String, Long), EventMessage[E]] =
    //   sql"select $readFields from $table where stream = $text and version > $int8 order by version asc"
    //     .query(readCodec)

    // def readStreamBefore: Query[(String, Long), EventMessage[E]] =
    //   sql"select $readFields from $table where stream = $text and version < $int8 order by version asc"
    //     .query(readCodec)
  }
}
