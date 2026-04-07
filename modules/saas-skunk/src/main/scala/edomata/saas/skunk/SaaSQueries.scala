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

package edomata.skunk

import _root_.skunk.*
import _root_.skunk.codec.all.*
import _root_.skunk.implicits.*
import edomata.backend.*
import edomata.backend.cqrs.AggregateState
import edomata.core.CommandMessage
import edomata.core.MessageMetadata
import java.time.OffsetDateTime
import java.time.ZoneOffset

/** SaaS-aware SQL queries with `tenant_id` and `owner_id` columns.
  *
  * These mirror `edomata.skunk.Queries` but add tenant/owner columns to the
  * states and outbox tables for multi-tenant isolation at the SQL level.
  */
private[skunk] object SaaSQueries {

  private[skunk] def setupSchema(namespace: String): Command[Void] =
    sql"""CREATE SCHEMA IF NOT EXISTS "#$namespace";""".command

  /** States table with tenant_id and owner_id columns. */
  final class State[S](
      naming: PGNaming,
      codec: BackendCodec[S]
  ) {
    private val t = naming.table("states")
    private val table = sql"""#$t"""
    private val state = codec.codec
    private val pk = naming.constraint("states_pk")
    private val tenantIdx = naming.index("states_tenant_idx")
    private val tenantOwnerIdx = naming.index("states_tenant_owner_idx")

    val setup: Command[Void] = sql"""
CREATE TABLE IF NOT EXISTS #$t (
  id text NOT NULL,
  "version" int8 NOT NULL,
  state #${codec.oid.name} NOT NULL,
  tenant_id text NOT NULL,
  owner_id text NOT NULL,
  CONSTRAINT #$pk PRIMARY KEY (id)
);
CREATE INDEX IF NOT EXISTS #$tenantIdx ON #$t (tenant_id);
CREATE INDEX IF NOT EXISTS #$tenantOwnerIdx ON #$t (tenant_id, owner_id);
""".command

    private def aggregateStateCodec: Codec[AggregateState[S]] =
      (state *: int8).to

    /** Upsert state with tenant_id and owner_id.
      *
      * Parameters: (id, state, tenantId, ownerId, expectedVersion)
      */
    def put: Command[String *: S *: String *: String *: SeqNr *: EmptyTuple] =
      sql"""
insert into $table (id, state, "version", tenant_id, owner_id) values ($text, $state, 1, $text, $text)
on conflict (id) do update
set version   = $table.version + 1,
    state     = excluded.state,
    tenant_id = excluded.tenant_id,
    owner_id  = excluded.owner_id
where $table.version = $int8
         """.command

    def get: Query[String, AggregateState[S]] =
      sql"""select state , version from $table where id = $text""".query(
        aggregateStateCodec
      )

    def listByTenant: Query[String, AggregateState[S]] =
      sql"""select state, version from $table where tenant_id = $text"""
        .query(aggregateStateCodec)
  }

  /** Outbox table with tenant_id column. */
  final class Outbox[N](naming: PGNaming, codec: BackendCodec[N]) {
    private val t = naming.table("outbox")
    private val table = sql"""#$t"""
    private val notification = codec.codec
    private val pk = naming.constraint("outbox_pk")
    private val tenantIdx = naming.index("outbox_tenant_idx")

    val setup: Command[Void] = sql"""
CREATE TABLE IF NOT EXISTS #$t(
  seqnr bigserial NOT NULL,
  stream text NOT NULL,
  correlation text NULL,
  causation text NULL,
  payload #${codec.oid.name} NOT NULL,
  created timestamptz NOT NULL,
  published timestamptz NULL,
  tenant_id text NOT NULL,
  CONSTRAINT #$pk PRIMARY KEY (seqnr)
);
CREATE INDEX IF NOT EXISTS #$tenantIdx ON #$t (tenant_id);
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

    /** Batch insert type: (notification, stream, time, metadata, tenantId) */
    type BatchInsert =
      List[(N, String, OffsetDateTime, MessageMetadata, String)]

    private val insertCodec =
      (notification *: text *: timestamptz *: metadata *: text)
    def insertAll(items: BatchInsert): Command[items.type] =
      sql"""
insert into $table (payload, stream, created, correlation, causation, tenant_id) values ${insertCodec.values
          .list(items)}
""".command
  }

  /** Commands table — reuses the standard schema (no tenant column needed). */
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

    private val instant: Codec[java.time.Instant] =
      timestamptz.imap(_.toInstant)(_.atOffset(ZoneOffset.UTC))

    private val command: Encoder[CommandMessage[?]] =
      (text *: text *: instant).contramap(c => (c.id, c.address, c.time))

    def insert: Command[CommandMessage[?]] = sql"""
insert into $table (id, address, "time") values ($command);
""".command
  }
}
