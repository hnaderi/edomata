package edfsm.core

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

  private def necOf[T](g: Gen[T]): Gen[NonEmptyChain[T]] =
    Gen
      .chooseNum(1, 10)
      .flatMap(n =>
        Gen
          .listOfN(n, g)
          .map(NonEmptyChain.fromSeq)
          .flatMap {
            case Some(e) => Gen.const(e)
            case None    => Gen.fail
          }
      )

}
