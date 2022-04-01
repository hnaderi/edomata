package edomata.core

import cats.Monad
import cats.data.NonEmptyChain
import cats.implicits.*
import cats.kernel.laws.discipline.EqTests
import cats.laws.discipline.MonadErrorTests
import cats.laws.discipline.MonadTests
import cats.laws.discipline.arbitrary.catsLawsCogenForNonEmptyChain
import munit.*
import org.scalacheck.Arbitrary
import org.scalacheck.Gen
import org.scalacheck.Prop.forAll

import ResponseMonadSuite.*
import DecisionTest.*

class ResponseMonadSuite extends DisciplineSuite {
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
    "laws",
    MonadTests[Res].monad[Int, Int, String]
  )

  checkAll("laws", EqTests[Res[Long]].eqv)

  property("Accumulates on accept") {
    forAll(notRejected, notifications, notRejected, notifications) {
      (a1, n1, a2, n2) =>
        val r1 = ResponseMonad(a1, n1)
        val r2 = ResponseMonad(a2, n2)
        val r3 = r1 >> r2
        assertEquals(r3, ResponseMonad(a1.flatMap(_ => a2), n1 ++ n2))
    }
  }
  property("Resets on rejection") {
    forAll(notRejected, notifications, rejected, notifications) {
      (a1, n1, a2, n2) =>
        val r1 = ResponseMonad(a1, n1)
        val r2 = ResponseMonad(a2, n2)
        val r3 = r1 >> r2
        assertEquals(r3, ResponseMonad(a1.flatMap(_ => a2), n2))
        assert(r3.result.isRejected)
    }
  }
  property("Rejected does not change") {
    forAll(rejected, notifications, anySut, notifications) { (a1, n1, a2, n2) =>
      val r1 = ResponseMonad(a1, n1)
      val r2 = ResponseMonad(a2, n2)
      val r3 = r1 >> r2
      assertEquals(r3, r1)
      assert(r3.result.isRejected)
    }
  }
  property("Publish on rejection") {
    forAll(rejected, notifications, notifications) { (a1, n1, n2) =>
      val r1 = ResponseMonad(a1, n1)
      val r2 = r1.publishOnRejection(n2: _*)

      assertEquals(
        r2,
        r1.copy(notifications = r1.notifications ++ n2)
      )
    }
  }
  property("Publish adds notifications") {
    forAll(anySut, notifications, notifications) { (a1, n1, n2) =>
      val r1 = ResponseMonad(a1, n1)
      val r2 = r1.publish(n2: _*)
      assertEquals(r2, r1.copy(notifications = n1 ++ n2))
    }
  }
  property("Reset clears notifications") {
    forAll(anySut, notifications) { (a1, n1) =>
      val r1 = ResponseMonad(a1, n1)
      val r2 = r1.reset
      assertEquals(r2.notifications, Nil)
      assertEquals(r2.result, r1.result)
    }
  }
}

object ResponseMonadSuite {
  val notifications: Gen[Seq[Notification]] =
    Gen.containerOf[Seq, Notification](
      Arbitrary.arbitrary[String].map(Notification(_))
    )
}
