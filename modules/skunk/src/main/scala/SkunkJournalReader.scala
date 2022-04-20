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
import fs2.Stream
import skunk.Codec
import skunk.Session
import skunk.data.Identifier

import java.time.OffsetDateTime
import java.time.ZoneOffset
import scala.concurrent.duration.*

private final class SkunkJournalReader[F[_]: Concurrent, E](
    pool: Resource[F, Session[F]],
    q: Queries.Journal[E]
) extends JournalReader[F, E] {

  private def run[A, B](q: skunk.Query[A, B])(a: A) =
    Stream.resource(pool.flatMap(_.prepare(q))).flatMap(_.stream(a, 100))

  def readStream(streamId: StreamId): Stream[F, EventMessage[E]] =
    run(q.readStream)(streamId)

  def readStreamAfter(
      streamId: StreamId,
      version: EventVersion
  ): Stream[F, EventMessage[E]] = run(q.readStreamAfter)((streamId, version))

  def readStreamBefore(
      streamId: StreamId,
      version: EventVersion
  ): Stream[F, EventMessage[E]] = run(q.readStreamBefore)((streamId, version))

  def readAll: Stream[F, EventMessage[E]] = run(q.readAll)(skunk.Void)

  def readAllAfter(seqNr: SeqNr): Stream[F, EventMessage[E]] =
    run(q.readAllAfter)(seqNr)

  def notifications: Stream[F, StreamId] = ???
}
