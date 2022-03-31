package edomata.core

import cats.Monad
import cats.data.NonEmptyChain
import cats.implicits.*
import munit.*
import org.scalacheck.Arbitrary
import org.scalacheck.Gen
import org.scalacheck.Prop.forAll

import DecisionTest.*

class DecisionTest extends ScalaCheckSuite {
  property("Accepted accumulates") {
    forAll(accepted, accepted) { (a, b) =>
      val c = a.flatMap(_ => b)

      assertEquals(c, Decision.Accepted(a.events ++ b.events, b.result))
    }
  }

  test("tail rec") {
    val c = Monad[[t] =>> Decision[Rejection, Event, t]].tailRecM(0) { a =>
      if (a < 10) then Decision.acceptReturn(Left(a + 1))(a)
      else Decision.pure(a.asRight)
    }
    assertEquals(c, Decision.Accepted(NonEmptyChain(0, (1 to 9): _*), 10))
  }

  property("Rejected terminates") {
    forAll(notRejected, rejected) { (a, b) =>
      val c = a.flatMap(_ => b)

      assertEquals(c, b)
    }
  }
  property("Rejected does not change") {
    forAll(rejected, notRejected) { (a, b) =>
      val c = a.flatMap(_ => b)

      assertEquals(c, a)
    }
  }
}

object DecisionTest {
  type Rejection = String
  type Event = Int

  type SUT = Decision[Rejection, Event, Long]

  val accepted: Gen[Decision.Accepted[Rejection, Event, Long]] = for {
    v <- Arbitrary.arbitrary[Long]
    l <- necOf(Arbitrary.arbitrary[Int])
  } yield Decision.Accepted(l, v)

  val rejected: Gen[Decision.Rejected[Rejection, Event, Long]] =
    necOf(Arbitrary.arbitrary[String]).map(Decision.Rejected(_))

  val indecisive: Gen[Decision.InDecisive[Rejection, Event, Long]] =
    Arbitrary.arbitrary[Long].map(Decision.InDecisive(_))

  val anySut: Gen[SUT] = Gen.oneOf(accepted, rejected, indecisive)
  val notRejected: Gen[SUT] = Gen.oneOf(accepted, indecisive)
}
