package edomata.backend

import cats.data.EitherNec
import cats.data.NonEmptyChain
import cats.effect.Concurrent
import cats.effect.implicits.*
import cats.effect.kernel.Async
import cats.effect.kernel.Clock
import cats.effect.kernel.Resource
import cats.effect.kernel.Temporal
import cats.implicits.*
import edomata.core.*
import fs2.Chunk
import fs2.Stream
import skunk.Codec
import skunk.Session
import skunk.data.Identifier

import java.time.OffsetDateTime
import java.time.ZoneOffset
import scala.concurrent.duration.*

private final class SkunkSnapshotPersistence[F[_]: Concurrent, S](
    pool: Resource[F, Session[F]],
    q: Queries.Snapshot[S]
) extends SnapshotPersistence[F, S] {
  def get(id: StreamId): F[Option[AggregateState.Valid[S]]] =
    pool.flatMap(_.prepare(q.get)).use(_.option(id))
  def put(items: Chunk[SnapshotItem[S]]): F[Unit] =
    val l = items.toList
    pool.flatMap(_.prepare(q.put(l))).use(_.execute(l)).void
}

private object SkunkSnapshotPersistence {
  def apply[F[_]: Concurrent, S](
      pool: Resource[F, Session[F]],
      namespace: PGNamespace
  )(using codec: BackendCodec[S]) =
    val q = Queries.Snapshot[S](namespace, codec)
    pool.use(_.execute(q.setup)).as(new SkunkSnapshotPersistence(pool, q))
}
