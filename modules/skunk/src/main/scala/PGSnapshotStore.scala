package edfsm.eventsourcing

import cats.Functor
import cats.Monad
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
import skunk.*
import skunk.circe.codec.all.jsonb
import skunk.codec.all.*
import skunk.data.Completion
import skunk.implicits.*

import java.time.OffsetDateTime

object PGSnapshotStore {
  final case class Queries(namespace: String) {
    private val escName = s""""$namespace""""
    val table = sql"#$escName.snapshots"
    val createTable: Command[Void] = sql"""
CREATE TABLE IF NOT EXISTS $table (
  id text NOT NULL,
  "version" int8 NOT NULL,
  state jsonb NOT NULL,
  CONSTRAINT snapshots_pk PRIMARY KEY (id)
);
""".command

    def aggregateStateCodec[S](using
        codec: Codec[S]
    ): Codec[AggregateState[S]] =
      (int8 *: codec).pimap

    def put[S](using
        codec: Codec[AggregateState[S]]
    ): Command[(String, AggregateState[S])] =
      sql"""insert into $table (id, "version", state) values ($text, $codec)
            on conflict (id) do update
                                set version = excluded.version,
                                    state   = excluded.state
         """.command

    def get[S](using
        codec: Codec[AggregateState[S]]
    ): Query[String, AggregateState[S]] =
      sql"""select version, state from $table where id = $text""".query(
        codec
      )
  }

  def setup[F[_]: Functor](namespace: String, session: Session[F]): F[Unit] =
    session.execute(Queries(namespace).createTable).void

  def apply[F[_], S: io.circe.Encoder: io.circe.Decoder](
      namespace: String,
      session: Session[F]
  )(using
      MonadCancel[F, Throwable]
  ): SnapshotStore[F, String, AggregateState[S]] =
    val queries = Queries(namespace)
    given Codec[S] = jsonb[S]
    given Codec[AggregateState[S]] = queries.aggregateStateCodec
    new SnapshotStore {
      def get(id: String): F[Option[AggregateState[S]]] =
        session.prepare(queries.get[S]).use(_.option(id))
      def put(id: String, state: AggregateState[S]): F[Unit] =
        session.prepare(queries.put[S]).use(_.execute((id, state)).void)
    }
}
