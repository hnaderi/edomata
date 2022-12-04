package edomata.core

import cats.Monad
import cats.data.*
import cats.implicits.*
import cats.kernel.laws.discipline.EqTests
import cats.laws.discipline.MonadTests
import cats.laws.discipline.TraverseTests
import cats.laws.discipline.arbitrary.catsLawsArbitraryForNonEmptyChain
import cats.laws.discipline.arbitrary.catsLawsCogenForNonEmptyChain
import munit.*
import org.scalacheck.Arbitrary
import org.scalacheck.Gen
import org.scalacheck.Prop.forAll

import Generators.*

private given Arbitrary[Notification] = Arbitrary(
  Arbitrary.arbitrary[String].map(Notification(_))
)
class ResponseDecisionSuite
    extends ResponseTLaws[Decision[*, Event, *], Rejection, Long, Notification](
      rejected,
      notRejected
    ) {
  otherLaws(
    TraverseTests[App].traverse[Int, Int, Int, Set[Int], Option, Option]
  )
}

class ResponseEitherNecSuite
    extends ResponseTLaws[EitherNec, Rejection, Long, Notification](
      Arbitrary.arbitrary[NonEmptyChain[Rejection]].map(Left(_)),
      Arbitrary.arbitrary[Long].map(Right(_))
    ) {
  otherLaws(
    TraverseTests[App].traverse[Int, Int, Int, Set[Int], Option, Option]
  )
}
