package edfsm.eventsourcing

import cats.data.NonEmptyChain
import cats.effect.Async
import cats.effect.Concurrent
import cats.effect.Resource
import cats.effect.kernel.Ref
import cats.effect.std.Semaphore
import cats.implicits.*
import fs2.Stream
import fs2.Stream.*

import java.time.OffsetDateTime
import java.util.UUID

object InMemJournal {
  private type State[E] = Map[String, List[EventMessage[E]]]

  enum Error(msg: String) extends Exception(msg) {
    case Conflict(current: SeqNr, attempt: SeqNr)
        extends Error(
          s"Rejected attempt to append with version: $attempt, while on version: $current"
        )
  }

  def apply[F[_], I <: String, E](using F: Async[F]): F[Journal[F, I, E]] =
    for {
      state <- Ref[F].of[State[E]](Map.empty)
    } yield new Journal[F, I, E] {
      val uuid = F.delay(UUID.randomUUID)

      def append(
          streamId: I,
          time: OffsetDateTime,
          version: SeqNr,
          events: NonEmptyChain[E]
      ): F[Unit] =
        val meta = (v: SeqNr) =>
          uuid.map(EventMetadata(_, time, v, v, streamId))
        val toAppend = events.zipWithIndex.toList.traverse((e, i) =>
          meta(i + version + 1).map(EventMessage(_, e))
        )
        toAppend.flatMap { evs =>
          state
            .modify { m =>
              val stream = m.getOrElse(streamId, Nil)
              val currentVersion = stream.size

              if currentVersion == version then
                val newS = stream ++ evs
                (m.updated(streamId, newS), ().asRight)
              else (m, Error.Conflict(currentVersion, version).asLeft)
            }
            .flatMap(F.fromEither)
        }

      def readStream(streamId: I): Stream[F, EventMessage[E]] =
        eval(state.get.map(_.get(streamId))).unNone.flatMap(emits)

      def readStreamAfter(
          streamId: I,
          version: EventVersion
      ): Stream[F, EventMessage[E]] = readStream(streamId).drop(version)

      def readAll: Stream[F, EventMessage[E]] = evalSeq(
        state.get.map(_.values.toList.flatten)
      )

      def readAllAfter(seqNr: SeqNr): Stream[F, EventMessage[E]] = readAll

      def notifications: Stream[F, I] = Stream.empty
    }
}
