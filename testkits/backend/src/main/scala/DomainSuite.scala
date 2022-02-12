package edfsm.backend.test

import cats.Eval
import cats.data.EitherNec
import cats.data.NonEmptyChain
import cats.effect.IO
import cats.implicits.*
import edfsm.backend.FSMDefinition.*
import edfsm.backend.RequestContext
import edfsm.backend.TestSystem
import edfsm.backend.TestSystem.TestResult
import edfsm.backend.*
import edfsm.core.Decision
import munit.CatsEffectSuite
import edfsm.backend.CommandMessage

import java.time.Instant

trait DomainSuite[Domain](
    transition: DomainTransition[Domain],
    logic: DomainLogic[IO, Domain],
    default: StateFor[Domain]
) extends CatsEffectSuite,
      munit.ScalaCheckSuite,
      munit.ScalaCheckEffectSuite {
  protected val dummy = TestSystem.handle(transition, logic)

  def request(
      cmd: CommandFor[Domain],
      state: StateFor[Domain] = default,
      address: String = "abc",
      id: String = "1",
      time: Instant = Instant.MIN
  ): RequestContext[Domain] =
    RequestContext(
      aggregateId = address,
      command = CommandMessage(
        id,
        time,
        address,
        cmd
      ),
      state = state
    )

  def send(req: RequestContext[Domain]): IO[TestResult[Domain]] = dummy(req)
}
