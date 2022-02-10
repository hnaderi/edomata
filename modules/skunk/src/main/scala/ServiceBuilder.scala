package edfsm.backend

import cats.effect.*
import cats.effect.implicits.*
import cats.implicits.*
import edfsm.eventsourcing.*
import io.circe.Codec
import io.circe.Decoder
import io.circe.Encoder
import io.odin.Logger
import io.odin.syntax.*
import natchez.Trace
import edfsm.common.pgPersistence.Database
import skunk.Session

import FSMDefinition.*

/** Builder for constructing a FSM service handler in F
  */
final class ServiceBuilder[F[_]: Async: Trace](
    pool: Resource[F, Session[F]],
    logger: Logger[F]
) {

  /** Builds a domain service for a Domain
    * @param name
    *   service name used as a namespace
    * @param initial
    *   initial state for each domain entity
    * @param fold
    *   definition of domain transitions
    * @param logic
    *   domain decider
    */
  def build[Domain](
      name: String,
      initial: StateFor[Domain],
      fold: DomainTransition[Domain],
      logic: DomainLogic[F, Domain]
  )(using
      Encoder[CommandFor[Domain]],
      Encoder[StateFor[Domain]],
      Decoder[StateFor[Domain]],
      Encoder[InternalEventFor[Domain]],
      Decoder[InternalEventFor[Domain]],
      Encoder[ExternalEventFor[Domain]],
      Decoder[ExternalEventFor[Domain]]
  ): F[CommandHandler[F, Domain]] =
    val d = PGServiceDAL.from(name, initial, fold, logger)(pool)
    pool.use(s =>
      s.execute(Database.Schema.create(name)) >>
        PGServiceDAL.setup[F, Domain](name, logger)(s)
    ) >>
      CommandHandler(d, logic, logger)
}

object ServiceBuilder {
  def default[F[_]: Async: Trace](
      pool: Resource[F, Session[F]]
  ): ServiceBuilder[F] =
    withLogger(pool, io.odin.consoleLogger())

  def withLogger[F[_]: Async: Trace](
      pool: Resource[F, Session[F]],
      logger: Logger[F]
  ): ServiceBuilder[F] = ServiceBuilder[F](pool, logger)
}
