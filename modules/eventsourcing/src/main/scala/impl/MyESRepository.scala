package edomata.eventsourcing

import cats.Monad
import cats.MonadError
import cats.Show
import cats.data.EitherNec
import cats.data.NonEmptyChain
import cats.effect.Concurrent
import cats.implicits.*
import fs2.Pipe
import fs2.Stream
import fs2.Stream.*
import io.odin.Logger

import java.time.OffsetDateTime
import java.util.UUID

import MyESRepository.*

final class MyESRepository[F[_]: Concurrent, I <: String, E, S, R](
    snapshot: SnapshotStore[F, I, AggregateState[S]],
    journal: Journal[F, I, E],
    logger: Logger[F],
    initial: S,
    fold: Fold[S, E, R]
)(using Show[R])
    extends ESRepository[F, I, E, S] {
  def append(
      streamId: I,
      time: OffsetDateTime,
      version: SeqNr,
      events: NonEmptyChain[E]
  ): F[Unit] =
    for {
      _ <- logger.trace(
        "Appending",
        Map("stream" -> streamId, "version" -> version.toString)
      )
      s <- get(streamId)
      // Validate projection before appending to journal
      newS <- MonadError[F, Throwable].fromEither(
        events
          .foldLeft(Right(s))(vFold)
          .leftMap(errs => Error.LogicError(errs.map(_.show)))
      )
      _ <- journal.append(streamId, time, version, events)
      _ <- logger.trace("Updating snapshot")
      _ <- snapshot.put(streamId, newS)
    } yield ()

  private val nuuid = new UUID(0L, 0L)
  private val nEvMsg = Right(
    EventMessage(
      EventMetadata(
        id = nuuid,
        time = OffsetDateTime.MIN,
        seqNr = 0L,
        version = 0L,
        stream = ""
      ),
      initial
    )
  )

  def history(streamId: I): Stream[F, EventMessage[S]] =
    journal
      .readStream(streamId)
      .scan(nEvMsg)(hFold)
      .through(unWrap)
      .tail

  def get(streamId: I): F[AggregateState[S]] = for {
    sn <- snapshot.get(streamId)
    _ <- logger.trace(s"read snapshot $sn")
    s <- sn
      .map(rehydrateUsing(streamId, _).widen)
      .getOrElse(rehydrateAll(streamId))
    _ <- snapshot.put(streamId, s)
  } yield s

  private def rehydrateAll(stream: I): F[AggregateState[S]] =
    logger.trace("Rehydrating all events", Map("stream" -> stream)) >>
      rehydrateUsing(stream, AggregateState(0, initial))

  private type FF[S, E] = (S, E) => S

  private def goToNewState(s: S, event: EventMessage[E]): Either[Error, S] =
    fold(event.payload)(s)
      .leftMap(errs => Error.FoldError(event.metadata, errs.map(_.show)))
      .toEither

  private val vFold: FF[EitherNec[R, AggregateState[S]], E] =
    (lastTransition, event) =>
      for {
        last <- lastTransition
        newState <- fold(event)(last.state).toEither
      } yield AggregateState(version = last.version + 1, state = newState)

  private val sFold: FF[Either[Error, AggregateState[S]], EventMessage[E]] =
    (lastTransition, event) =>
      for {
        last <- lastTransition
        newState <- goToNewState(last.state, event)
      } yield AggregateState(version = event.metadata.version, state = newState)

  private val hFold: FF[Either[Error, EventMessage[S]], EventMessage[E]] =
    (lastTransition, event) =>
      for {
        last <- lastTransition
        newState <- goToNewState(last.payload, event)
      } yield event.copy(payload = newState)

  private def rehydrateUsing(
      stream: I,
      initial: AggregateState[S]
  ): F[AggregateState[S]] =
    logger.trace(
      "Partial rehydrating",
      Map("stream" -> stream, "after" -> initial.version.toString)
    ) >>
      journal
        .readStreamAfter(stream, initial.version)
        .scan(Right(initial))(sFold)
        .through(unWrap)
        .compile
        .lastOrError

  def unWrap[T]: Pipe[F, Either[Error, T], T] = _.flatMap {
    case Right(s)  => emit(s)
    case Left(err) => raiseError(err)
  }
}

object MyESRepository {
  def apply[F[_]: Concurrent, I <: String, E, S, R](
      snapshot: SnapshotStore[F, I, AggregateState[S]],
      journal: Journal[F, I, E],
      logger: Logger[F],
      initial: S,
      fold: Fold[S, E, R]
  )(using Show[R]): MyESRepository[F, I, E, S, R] =
    new MyESRepository(snapshot, journal, logger, initial, fold)

  enum Error(msg: String) extends Exception(msg) {
    case FoldError(metadata: EventMetadata, debug: NonEmptyChain[String])
        extends Error(
          s"""Error while folding events! this is a fatal programming error.
event:
  id = ${metadata.id}
  stream = ${metadata.stream}
debug:
${debug.mkString_("\n")}
"""
        )
    case LogicError(debug: NonEmptyChain[String])
        extends Error(
          s"Fatal programming error! debug: ${debug.mkString_("\n")}"
        )
  }
}
