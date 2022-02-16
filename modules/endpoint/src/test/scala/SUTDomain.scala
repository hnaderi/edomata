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

object SUTDomain {
  def logic: DomainLogic[IO, SUTDomain] = req =>
    val inp = req.command.payload
    if inp % 2 == 0 then Action.accept(inp)
    else Action.reject(s"Cannot add odd number $inp", "some other error")

  def fold: DomainTransition[SUTDomain] = i => s => (i + s).validNec
}

type SUTDomain = (
    HasCommand[Int],
    HasRejection[String],
    HasState[Long],
    HasInternalEvent[Int],
    HasExternalEvent[Long]
)
