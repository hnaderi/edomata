package edfsm.eventsourcing

import fs2.Stream
import fs2.Stream.*
import cats.effect.Concurrent
import fs2.Pipe
import cats.effect.std.Queue
import _root_.io.odin.Logger
import cats.effect.kernel.Ref

trait JournalConsumer[F[_], E] {
  def commit(seqNr: SeqNr): F[Unit]
  def consume: Stream[F, EventMessage[E]]
}

@deprecated("Not ready to use in production")
object JournalConsumer {
  def from[F[_]: Concurrent, I, E](
      controller: ConsumerController[F],
      journal: Journal[F, I, E],
      logger: Logger[F],
      consumerName: String
  ): JournalConsumer[F, E] = new JournalConsumer[F, E] {
    val ctx = Map("consumer" -> consumerName)

    def commit(seqNr: SeqNr): F[Unit] = controller.seek(consumerName, seqNr)

    def consume: Stream[F, EventMessage[E]] = for {
      _ <- eval(logger.info("Starting", ctx))
      last <- eval(controller.read(consumerName))
      state <- eval(Ref[F].of(last))
      _ <- emit(()) ++ waitForNewEvent
      ev <- read(state)
    } yield ev

    private def read(state: Ref[F, Option[Long]]) =
      eval(state.get)
        .flatMap {
          case Some(lastOffset) =>
            eval(logger.debug(s"continuing from $lastOffset")) >>
              journal.readAllAfter(lastOffset)
          case None =>
            eval(
              logger.info(s"start consuming all events from the beginning...")
            ) >>
              journal.readAll
        }
        .evalTap { ev =>
          state.set(Some(ev.metadata.seqNr))
        }

    def waitForNewEvent =
      eval(logger.info("Waiting for new event...", ctx)) >>
        journal.notifications.through(keepLatestOnly) >>
        eval(logger.info("Received notification for new event!", ctx))
  }

  private def keepLatestOnly[F[_]: Concurrent, T]: Pipe[F, T, T] = in =>
    for {
      q <- eval(Queue.circularBuffer[F, T](1))
      t <- repeatEval(q.take) concurrently in.evalMap(q.offer)
    } yield t
}
