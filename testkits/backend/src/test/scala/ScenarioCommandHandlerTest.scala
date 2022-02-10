package edfsm.backend

import cats.effect.IO
import cats.implicits.*
import munit.CatsEffectSuite
import munit.ScalaCheckEffectSuite
import org.scalacheck.Arbitrary
import org.scalacheck.Gen
import org.scalacheck.effect.PropF.*
import edfsm.protocols.command.CommandMessage

import java.time.Instant

import FSMDefinition.*
import ScenarioCommandHandlerTest.*

class ScenarioCommandHandlerTest
    extends CatsEffectSuite,
      ScalaCheckEffectSuite {

  test("must always add command to inbox") {
    forAllF(outcomes, commands) { (outcome, cmd) =>
      for {
        s <- ScenarioCommandHandler(timeline)
        handler <- s.sequence(outcome.once)
        _ <- handler(cmd).attempt
        _ <- s.inbox.assertEquals(List(cmd))
      } yield ()
    }
  }

  test("must abide destiny count") {
    val count = Gen.chooseNum(1, 10)
    forAllF(outcomes, commands, count) { (outcome, cmd, n) =>
      for {
        s <- ScenarioCommandHandler(timeline)
        handler <- s.sequence(outcome * n)
        _ <- (1 to n).toList.traverse(_ => handler(cmd).attempt)
        _ <- assertCommandCount(n, s, cmd)
        _ <- assertTerminal(handler, cmd)
      } yield ()
    }
  }

  test("must experience destiny sequences") {
    val count = Gen.chooseNum(1, 10)
    forAllF(outcomes, commands, count) { (outcome, cmd, n) =>
      for {
        s <- ScenarioCommandHandler(timeline)
        handler <- s.sequence(outcome * n, outcome.once)
        _ <- (1 to n + 1).toList.traverse(_ => handler(cmd).attempt)
        _ <- assertCommandCount(n + 1, s, cmd)
        _ <- assertTerminal(handler, cmd)
      } yield ()
    }
  }
  private def assertCommandCount(
      n: Int,
      s: ScenarioCommandHandler[timeline.type, Domain],
      cmd: CommandMessage[String]
  ) =
    s.inbox.assertEquals(List.fill(n)(cmd))

  private def assertTerminal(
      handler: CommandHandler[IO, Domain],
      cmd: CommandMessage[String]
  ) = handler(cmd).attempt.assertEquals(
    Left(ScenarioCommandHandler.Error.TimelineFinished)
  )

}

object ScenarioCommandHandlerTest {
  type Domain = (
      HasCommand[String],
      HasRejection[String],
      HasInternalEvent[String],
      HasExternalEvent[String],
      HasState[String]
  )

  private val timeline = Timeline[Domain]

  private val commands = for {
    id <- Arbitrary.arbitrary[String]
    address <- Arbitrary.arbitrary[String]
    pl <- Arbitrary.arbitrary[String]
    time <- Arbitrary.arbitrary[Instant]
  } yield CommandMessage(
    id = id,
    time = time,
    address = address,
    payload = pl
  )

  private val outcomes = Gen.oneOf(
    timeline.ok,
    timeline.fail,
    timeline.reject("Some error")
  )
}
