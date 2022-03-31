package edomata.core

import cats.Monad
import cats.data.NonEmptyChain
import cats.implicits.*
import cats.laws.discipline.FunctorTests
import cats.laws.discipline.MonadErrorTests
import cats.laws.discipline.arbitrary.catsLawsCogenForNonEmptyChain
import munit.*
import org.scalacheck.Arbitrary
import org.scalacheck.Gen
import org.scalacheck.Prop.forAll

import DecisionTest.*

class DecisionLawTest extends DisciplineSuite {
  private type D[T] = Decision[String, Int, T]
  private given [T: Arbitrary]: Arbitrary[D[T]] = Arbitrary(
    Arbitrary.arbitrary[T].flatMap(t => DecisionTest.anySut.map(_.as(t)))
  )
  private given Arbitrary[NonEmptyChain[String]] = Arbitrary(
    necOf(Arbitrary.arbitrary[String])
  )

  checkAll(
    "Decision monad error",
    MonadErrorTests[D, NonEmptyChain[String]].monadError[Int, Int, String]
  )
}
