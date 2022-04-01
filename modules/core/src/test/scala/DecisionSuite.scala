package edomata.core

import cats.Monad
import cats.data.NonEmptyChain
import cats.implicits.*
import cats.kernel.laws.discipline.EqTests
import cats.kernel.laws.discipline.SerializableTests
import cats.laws.discipline.FunctorTests
import cats.laws.discipline.MonadErrorTests
import cats.laws.discipline.arbitrary.catsLawsCogenForNonEmptyChain
import munit.*
import org.scalacheck.Arbitrary
import org.scalacheck.Gen
import org.scalacheck.Prop.forAll

import Generators.*
import cats.laws.discipline.TraverseTests

class DecisionSuite extends DisciplineSuite {
  private given [T: Arbitrary]: Arbitrary[D[T]] = Arbitrary(
    Arbitrary.arbitrary[T].flatMap(t => Generators.anySut.map(_.as(t)))
  )
  private given Arbitrary[NonEmptyChain[String]] = Arbitrary(
    necOf(Arbitrary.arbitrary[String])
  )

  checkAll(
    "laws",
    MonadErrorTests[D, NonEmptyChain[String]].monadError[Int, Int, String]
  )
  checkAll(
    "laws",
    TraverseTests[D].traverse[Int, Int, Int, Set[Int], Option, Option]
  )
  checkAll("laws", EqTests[D[Long]].eqv)
  checkAll("laws", SerializableTests.serializable[SUT](Decision.pure(123L)))

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
