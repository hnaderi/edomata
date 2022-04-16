package edomata.backend

import cats.data.EitherNec
import cats.data.NonEmptyChain
import cats.effect.Concurrent
import cats.effect.implicits.*
import cats.effect.kernel.Async
import cats.effect.kernel.Resource
import cats.effect.kernel.Temporal
import cats.implicits.*
import edomata.core.CommandMessage
import edomata.core.Compiler
import edomata.core.Domain
import edomata.core.ModelTC
import edomata.core.ProgramResult
import edomata.core.RequestContext
import fs2.Stream
import skunk.Codec
import skunk.Session

import scala.concurrent.duration.*

final class SkunkBackend[F[_], S, E, R, N] private (
    snapshot: SnapshotStore[F, S, E, R],
    pool: Resource[F, Session[F]],
    evQ: Queries.Journal[E],
    nQ: Queries.Outbox[N]
)(using
    m: ModelTC[S, E, R],
    F: Temporal[F]
) extends Backend[F, S, E, R, N] {
  private val jW: JournalWriter[F, E] = ???
  private val oW: OutboxWriter[F, N] = ???
  private val cmds: CommandStore[F] = ???

  def compiler[C]: Compiler[F, C, S, E, R, N] = new {
    def onRequest(cmd: CommandMessage[C])(
        run: RequestContext[C, S] => F[ProgramResult[S, E, R, N]]
    ): F[EitherNec[R, Unit]] = repository.get(cmd.address).flatMap {
      case AggregateState.Valid(s, rev) =>
        val ctx = cmd.buildContext(s)
        run(ctx).flatMap {
          ???
        }
      case AggregateState.Conflicted(ls, lev, errs) => errs.asLeft.pure
    }

  }

  lazy val outbox: OutboxReader[F, N] = SkunkOutboxReader(pool, nQ)
  lazy val journal: JournalReader[F, E] = SkunkJournalReader(pool, evQ)
  lazy val repository: Repository[F, S, E, R] = Repository(journal, snapshot)
}

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

private final class SkunkOutboxReader[F[_]: Concurrent, N](
    pool: Resource[F, Session[F]],
    q: Queries.Outbox[N]
) extends OutboxReader[F, N] {
  def read: Stream[F, OutboxItem[N]] = Stream
    .resource(pool.flatMap(_.prepare(q.read)))
    .flatMap(_.stream(skunk.Void, 100))

  def markAllAsSent(items: NonEmptyChain[OutboxItem[N]]): F[Unit] =
    pool.flatMap(_.prepare(q.publish)).use(???)
}

object SkunkBackend {
  def apply[F[_]: Async](pool: Resource[F, Session[F]]): PartialBuilder[F] =
    PartialBuilder(pool)

  import scala.compiletime.error
  final class PartialBuilder[F[_]: Async](pool: Resource[F, Session[F]]) {
    inline def builder[C, S, E, R, N](
        domain: Domain[C, S, E, R, N],
        inline namespace: String
    )(using
        model: ModelTC[S, E, R]
    ): DomainBuilder[F, C, S, E, R, N] =
      DomainBuilder(
        pool,
        domain,
        model,
        PGNamespace(namespace),
        Resource.eval(SnapshotStore.inMem(1000))
      )
  }

  final case class DomainBuilder[
      F[_]: Async,
      C,
      S,
      E,
      R,
      N
  ] private[backend] (
      private val pool: Resource[F, Session[F]],
      private val domain: Domain[C, S, E, R, N],
      private val model: ModelTC[S, E, R],
      private val namespace: PGNamespace,
      private val snapshot: Resource[F, SnapshotStore[F, S, E, R]],
      private val maxRetry: Int = 5,
      private val retryInitialDelay: FiniteDuration = 2.seconds
  ) {

    def persistedSnapshot(
        storage: SnapshotPersistence[F, S, E, R],
        maxInMem: Int = 1000,
        maxBuffer: Int = 100,
        maxWait: FiniteDuration = 1.minute
    ): DomainBuilder[F, C, S, E, R, N] = copy(snapshot =
      SnapshotStore.persisted(
        storage,
        size = maxInMem,
        maxBuffer = maxBuffer,
        maxWait
      )
    )

    def inMemSnapshot(
        maxInMem: Int = 1000
    ): DomainBuilder[F, C, S, E, R, N] =
      copy(snapshot = Resource.eval(SnapshotStore.inMem(maxInMem)))

    def withSnapshot(
        s: Resource[F, SnapshotStore[F, S, E, R]]
    ): DomainBuilder[F, C, S, E, R, N] = copy(snapshot = s)

    def withRetryConfig(
        maxRetry: Int = maxRetry,
        retryInitialDelay: FiniteDuration = retryInitialDelay
    ): DomainBuilder[F, C, S, E, R, N] =
      copy(maxRetry = maxRetry, retryInitialDelay = retryInitialDelay)

    def build(using
        event: BackendCodec[E],
        notifs: BackendCodec[N]
    ): Resource[F, SkunkBackend[F, S, E, R, N]] = ???
  }
}
