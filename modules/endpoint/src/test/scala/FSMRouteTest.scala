package edomata

import cats.data.EitherNec
import cats.data.NonEmptyChain
import cats.effect.IO
import cats.effect.Resource
import cats.implicits.*
import edomata.backend.FSMDefinition.*
import edomata.backend.TestSystem.TestUnit
import edomata.backend.*
import edomata.core.Action
import io.circe.Json
import munit.CatsEffectSuite
import org.http4s.HttpApp
import org.http4s.Response
import org.http4s.Status
import org.http4s.circe.CirceEntityCodec.*
import org.http4s.implicits.*
import org.http4s.client.dsl.io.given
import org.http4s.Method.POST
import org.http4s.implicits.given
import java.time.Instant

class FSMRouteTest extends CatsEffectSuite {

  final case class Context(
      system: TestSystem[IO, SUTDomain],
      server: HttpApp[IO]
  )

  private val SUT = ResourceSuiteLocalFixture(
    "System",
    Resource.eval(
      for {
        sys <- TestSystem[IO, SUTDomain](0, SUTDomain.fold, SUTDomain.logic)
        cl = routesFor(sys)
      } yield Context(sys, cl)
    )
  )
  override def munitFixtures = List(SUT)

  private def routesFor(sys: TestSystem[IO, SUTDomain]) =
    FSMRoute
      .of[IO, SUTDomain](sys.handler) {
        case "inc" => _.as[Int]
        case "dec" => _.as[Int].map(-_)
      }
      .orNotFound

  private def assertResponse(status: Status, body: Option[Json])(
      resp: IO[Response[IO]]
  ): IO[Unit] = resp.flatMap(r =>
    IO(assertEquals(r.status, status)) >>
      body.fold(assertIOBoolean(r.body.compile.toVector.map(_.isEmpty), ""))(
        assertIO(r.as[Json], _)
      )
  )

  test("ok response") {
    val req = POST(2, uri"/sut/inc?id=123")

    assertResponse(Status.Ok, body = None)(SUT().server.run(req)) >>
      assertIO(
        SUT().system.inspect("sut"),
        TestSystem.TestUnit[SUTDomain](2L, Nil, List(2))
      )

  }

  test("error response") {
    import edomata.backend.CommandMessage

    val req = POST(3, uri"/sut-error/inc?id=234")

    assertResponse(
      Status.NotAcceptable,
      Json
        .arr(
          Json.fromString("Cannot add odd number 3"),
          Json.fromString("some other error")
        )
        .some
    )(SUT().server.run(req)) >>
      assertIO(
        SUT().system.inspect("sut-error"),
        TestSystem.TestUnit[SUTDomain](0L, Nil, Nil)
      )

  }

  test("command not found response") {
    val req = POST(2, uri"/sut/foo?id=123")

    assertResponse(Status.NotFound, body = None)(
      SUT().server.run(req)
    ) >>
      assertIO(
        SUT().system.inspect("sut"),
        TestSystem.TestUnit[SUTDomain](2L, Nil, List(2))
      )

  }

  test("const response") {
    val req = POST(2, uri"/sut-const/apply?id=123")

    val constRouter =
      FSMRoute.const(SUT().system.handler, "apply")(_.as[Int]).orNotFound

    assertResponse(Status.Ok, body = None)(constRouter.run(req)) >>
      assertIO(
        SUT().system.inspect("sut-const"),
        TestSystem.TestUnit[SUTDomain](2L, Nil, List(2))
      )

  }
}
