package edomata.core

import cats.Applicative
import cats.Eval
import cats.data.NonEmptyChain
import cats.implicits.*
import munit.*
import org.scalacheck.Arbitrary
import org.scalacheck.Gen
import org.scalacheck.Prop.forAll

import DecisionTTest.*

class DecisionTTest extends ScalaCheckSuite {
  property("Accepted accumulates") {
    forAll(accepted, accepted) { case ((a, at), (b, bt)) =>
      val c = at.flatMap(_ => bt).run.value

      assertEquals(c, Decision.Accepted(a.events ++ b.events, b.result))
    }
  }
  property("Rejected terminates") {
    forAll(notRejected, rejected) { case ((a, at), (b, bt)) =>
      val c = at.flatMap(_ => bt).run.value

      assertEquals(c, b)
    }
  }
  property("Rejected does not change") {
    forAll(rejected, notRejected) { case ((a, at), (b, bt)) =>
      val c = at.flatMap(_ => bt).run.value

      assertEquals(c, a)
    }
  }
}

object DecisionTTest {

  type SUTT = DecisionT[Eval, Rejection, Event, Long]

  private def lift[T <: SUT](t: Gen[T]): Gen[(T, SUTT)] =
    t.map(d => (d, DecisionT.lift(d)(using Applicative[Eval])))

  val accepted =
    lift(Generators.accepted)

  val rejected =
    lift(Generators.rejected)

  val indecisive =
    lift(Generators.indecisive)

  val anySut = Gen.oneOf(accepted, rejected, indecisive)
  val notRejected = Gen.oneOf(accepted, indecisive)

}
