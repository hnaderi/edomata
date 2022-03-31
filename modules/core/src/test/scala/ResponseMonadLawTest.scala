package edomata.core

import cats.Monad
import cats.data.NonEmptyChain
import cats.implicits.*
import cats.laws.discipline.MonadErrorTests
import cats.laws.discipline.arbitrary.catsLawsCogenForNonEmptyChain
import munit.*
import org.scalacheck.Arbitrary
import org.scalacheck.Gen
import org.scalacheck.Prop.forAll

import ResponseMonadTest.*
import cats.laws.discipline.MonadTests

class ResponseMonadLawTest extends DisciplineSuite {
  private given [T: Arbitrary]: Arbitrary[Res[T]] = Arbitrary(
    for {
      n <- notifications
      t <- Arbitrary.arbitrary[T]
      d <- DecisionTest.anySut
    } yield ResponseMonad(d.as(t), n)
  )
  private given Arbitrary[NonEmptyChain[String]] = Arbitrary(
    necOf(Arbitrary.arbitrary[String])
  )

  checkAll(
    "Response monad",
    MonadTests[Res].monad[Int, Int, String]
  )
}
