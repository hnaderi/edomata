package edomata.core

import cats.data.NonEmptyChain
import org.scalacheck.Arbitrary
import org.scalacheck.Gen

object Generators {
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
