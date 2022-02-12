package edfsm.backend

import cats.Monad
import cats.MonadError
import cats.data.EitherNec
import cats.data.Kleisli
import cats.data.NonEmptyChain
import cats.effect.*
import cats.effect.implicits.*
import cats.effect.std.Random
import cats.effect.std.Semaphore
import cats.implicits.*
import edfsm.core.Decision
import edfsm.core.DecisionT
import edfsm.backend.CommandMessage
import edfsm.eventsourcing.*
import io.circe.Codec
import io.circe.Encoder
import io.odin.Logger
import io.odin.syntax.*

import java.time.OffsetDateTime
import java.time.ZoneOffset
import scala.concurrent.duration.*

import FSMDefinition.*

object CommandHandler {
  enum Error extends Throwable {
    case JournalError(val ex: Throwable)
    case MaxRetryExceeds(val ex: Throwable)
  }

  def apply[F[_]: Async, Domain](
      dal: Resource[F, ServiceDAL[F, Domain]],
      logic: DomainLogic[F, Domain],
      logger: Logger[F]
  ): F[CommandHandler[F, Domain]] =
    Random
      .scalaUtilRandom[F]
      .map(rand => handler(dal, logic, logger, rand))

  private def handler[F[_], Domain](
      sdal: Resource[F, ServiceDAL[F, Domain]],
      logic: DomainLogic[F, Domain],
      logger: Logger[F],
      random: Random[F]
  )(using F: Temporal[F]): CommandHandler[F, Domain] = {
    val accepted: CommandResult[Domain] = Right(())

    def retryJournalError[T](max: Int, interval: FiniteDuration)(
        f: F[T]
    ): F[T] =
      f.recoverWith { case e: Error.JournalError =>
        if max > 0 then
          for {
            _ <- logger.warn(
              s"Failed! retry (remained $max, after: $interval)"
            )
            j <- random.nextLongBounded(interval.toMillis).map(_.millis)
            res <- retryJournalError(max - 1, interval + j)(f)
              .delayBy(interval)
          } yield res
        else F.raiseError(Error.MaxRetryExceeds(e))
      }

    val service: CommandHandler[F, Domain] = cmd => {
      val id = cmd.address
      val log = logger.withConstContext(
        Map("cmd" -> cmd.id, "address" -> cmd.address)
      )
      sdal.use { dal =>

        def handleDecision(
            stream: String,
            version: Long,
            cmd: DomainCommand[Domain],
            decision: DomainDecision[Domain, Unit],
            now: OffsetDateTime,
            log: Logger[F]
        ): F[CommandResult[Domain]] = decision match {
          case Decision.Accepted(evs, _) =>
            log.debug("Accepted!") >>
              dal.appendCmdLog(cmd) >>
              dal.appendJournal(stream, now, version, evs).as(accepted)
          case Decision.InDecisive(_) =>
            log
              .debug(
                "Made no decision! nothing changed and command is ignored."
              )
              .as(accepted)
          case Decision.Rejected(reasons) =>
            log.info("Rejected command!").as(Left(reasons))
        }

        val process: DomainAction[F, Domain] = for {
          _ <- log.debug("Processing")
          last <- dal.readFromJournal(id)
          _ <- log.trace(s"Last state version: ${last.version}")
          rCtx = RequestContext(
            aggregateId = id,
            command = cmd,
            state = last.state
          )
          result <- logic(rCtx).run
          now <- Clock[F].realTimeInstant.map(_.atOffset(ZoneOffset.UTC))
          res <- dal
            .transaction(
              result.notifications
                .traverse(dal.outbox(_, now)) >>
                handleDecision(
                  stream = id,
                  version = last.version,
                  cmd,
                  result.decision,
                  now,
                  log
                )
            )
            .adaptErr { case e => Error.JournalError(e) }
        } yield res

        val ignoreRedundantCommand: DomainAction[F, Domain] =
          log.info("Redundant command ignored!").as(accepted)

        dal
          .containsCmd(cmd.id)
          .ifM(
            ignoreRedundantCommand,
            retryJournalError(5, 2.seconds)(process)
          )
      }
    }
    service
  }
}
