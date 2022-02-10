package edfsm.common.notification

import skunk.*
import skunk.circe.codec.all.jsonb
import skunk.codec.all.*
import skunk.data.Identifier
import skunk.implicits.*

import java.time.OffsetDateTime
import scala.concurrent.duration.*

private[notification] final class Queries[T](
    namespace: String,
    codec: Codec[T]
) {
  val table = s""""$namespace".outbox"""
  val createTable: Command[Void] = sql"""
CREATE TABLE IF NOT EXISTS #${table}(
  seqnr bigserial NOT NULL,
  payload jsonb NOT NULL,
  created timestamptz NOT NULL,
  published timestamptz NULL,
  CONSTRAINT outbox_pk PRIMARY KEY (seqnr)
);
""".command

  val publish: Command[(OffsetDateTime, Long)] = sql"""
update #${table}
set published = $timestamptz
where seqnr <= $int8
  and published is NULL
""".command

  val markAsPublished: Command[(OffsetDateTime, Long)] = sql"""
update #${table}
set published = $timestamptz
where seqnr = $int8
""".command

  val itemCodec: Codec[OutboxItem[T]] = (int8 *: timestamptz *: codec).pimap

  val read: Query[Void, OutboxItem[T]] =
    sql"""
select seqnr, created, payload
from #${table}
where published is NULL
order by seqnr asc
limit 10
""".query(itemCodec)

  val insert: Command[(T, OffsetDateTime)] = sql"""
insert into #${table} (payload, created) values ($codec, $timestamptz)
""".command

}
