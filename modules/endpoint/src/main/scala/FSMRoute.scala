package edfsm

import cats.Monad
import cats.Show
import cats.effect.Clock
import cats.implicits.*
import edfsm.backend.CommandHandler
import edfsm.backend.CommandResult
import edfsm.backend.FSMDefinition.CommandFor
import edfsm.backend.FSMDefinition.RejectionFor
import io.circe.Json
import io.circe.syntax.*
import org.http4s.*
import org.http4s.circe.CirceEntityEncoder.*
import org.http4s.dsl.Http4sDsl
import org.http4s.dsl.impl.QueryParamDecoderMatcher
import org.http4s.syntax.*
import edfsm.backend.CommandId
import edfsm.backend.CommandMessage

object FSMRoute {
  object CommandIdParam extends QueryParamDecoderMatcher[CommandId]("id")
  def of[F[_]: Monad, Domain](service: CommandHandler[F, Domain])(
      h: PartialFunction[String, Request[F] => F[CommandFor[Domain]]]
  )(using show: Show[RejectionFor[Domain]], clock: Clock[F]): HttpRoutes[F] =
    val dsl = new Http4sDsl[F] {}
    import dsl.*
    HttpRoutes.of {
      case req @ POST -> Root / address / command :? CommandIdParam(id) =>
        command match {
          case h(foo) =>
            for {
              payload <- foo.apply(req)
              now <- clock.realTimeInstant
              cmd = CommandMessage(
                id = id,
                time = now,
                address = address,
                payload = payload
              )
              res <- service.apply(cmd)
              resp <- res match {
                case Right(_) => Ok()
                case Left(reasons) =>
                  NotAcceptable(reasons.map(show.show).asJson)
              }
            } yield resp
          case other => NotFound()
        }
    }

  def const[F[_]: Monad, Domain](
      service: CommandHandler[F, Domain],
      str: String
  )(
      h: Request[F] => F[CommandFor[Domain]]
  )(using show: Show[RejectionFor[Domain]], clock: Clock[F]): HttpRoutes[F] =
    of(service) { case str => h }

  def gateway[F[_]: Monad, Domain](
      service: CommandHandler[F, Domain]
  )(
      h: Request[F] => F[CommandFor[Domain]]
  )(using show: Show[RejectionFor[Domain]], clock: Clock[F]): HttpRoutes[F] =
    const(service, "apply")(h)
}
