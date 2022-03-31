package edomata.core

import cats.Eval
import cats.Monad
import cats.MonadError
import cats.data.Kleisli
import cats.data.NonEmptyChain
import cats.implicits.*
import munit.*
import org.scalacheck.Arbitrary.arbitrary
import org.scalacheck.Gen
import org.scalacheck.Prop.forAll

import ResponseMonadTest.*
import DecisionTest.*

class ResponseMonadTest extends FunSuite, ScalaCheckSuite {
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

  test("tail rec") {
    val n = 10
    val c = Monad[Res].tailRecM(0) { a =>
      if (a < n) then ResponseMonad.acceptReturn(Left(a + 1))(a)
      else ResponseMonad.pure(a.asRight)
    }
    assertEquals(c, ResponseMonad.acceptReturn(n)(0, (1 to n - 1): _*))
  }
}

object ResponseMonadTest {
  final case class Notification(value: String = "")

  type Res[T] = ResponseMonad[Rejection, Event, Notification, T]
  val notifications: Gen[Seq[Notification]] =
    Gen.containerOf[Seq, Notification](arbitrary[String].map(Notification(_)))
}
