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

private final class SkunkOutboxReader[F[_]: Concurrent: Clock, N](
    pool: Resource[F, Session[F]],
    q: Queries.Outbox[N]
) extends OutboxReader[F, N] {
  def read: Stream[F, OutboxItem[N]] = Stream
    .resource(pool.flatMap(_.prepare(q.read)))
    .flatMap(_.stream(skunk.Void, 100))

  def markAllAsSent(items: NonEmptyChain[OutboxItem[N]]): F[Unit] = for {
    now <- currentTime[F]
    is = items.toList.map(_.seqNr)
    _ <- pool
      .flatMap(_.prepare(q.markAsPublished(is)))
      .use(_.execute((now, is)))
  } yield ()
}

object SkunkBackend {
  def apply[F[_]: Async](pool: Resource[F, Session[F]]): PartialBuilder[F] =
    PartialBuilder(pool)

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

    private def _setup(using
        event: BackendCodec[E],
        notifs: BackendCodec[N]
    ) = {
      val jQ = Queries.Journal(namespace, event)
      val nQ = Queries.Outbox(namespace, notifs)
      val cQ = Queries.Commands(namespace)

      pool
        .use(s =>
          s.execute(Queries.setupSchema(namespace)) >>
            s.execute(jQ.setup) >> s.execute(nQ.setup) >> s.execute(cQ.setup)
        )
        .as((jQ, nQ, cQ))
    }

    def setup(using
        event: BackendCodec[E],
        notifs: BackendCodec[N]
    ): F[Unit] = _setup.void

    def build(using
        event: BackendCodec[E],
        notifs: BackendCodec[N]
    ): Resource[F, Backend[F, S, E, R, N]] = for {
      qs <- Resource.eval(_setup)
      (jQ, nQ, cQ) = qs
      given ModelTC[S, E, R] = model
      s <- snapshot
      _outbox = SkunkOutboxReader(pool, nQ)
      _journal = SkunkJournalReader(pool, jQ)
      _repo = RepositoryReader(_journal, s)
      cmds <- Resource.eval(CommandStore.inMem(100))
      compiler = SkunkRepository(pool, jQ, nQ, cQ, cmds, s, _repo)
    } yield new {
      def compile[C](
          app: Edomaton[F, RequestContext[C, S], R, E, N, Unit]
      ): DomainService[F, CommandMessage[C], R] = cmd =>
        CommandHandler.retry(maxRetry, retryInitialDelay) {
          CommandHandler(compiler, app).apply(cmd)
        }
      val outbox: OutboxReader[F, N] = _outbox
      val journal: JournalReader[F, E] = _journal
      val repository: RepositoryReader[F, S, E, R] = _repo

    }
  }
}
