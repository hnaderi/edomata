package edomata.core

import cats.Eval
import cats.data.Kleisli
import cats.data.NonEmptyChain
import cats.implicits.*
import munit.*

import ActionTest.*
import org.scalacheck.Gen
import org.scalacheck.Prop.forAll
import org.scalacheck.Arbitrary

class ActionTest extends FunSuite, ScalaCheckSuite {
  test("Empty action") {
    val a: SUT[Unit] = Action.void

    val res = a.run.value

    assertEquals(res.decision, Decision.InDecisive(()))
    assert(res.notifications.isEmpty, "Non empty notifications!")
  }

  test("Notification") {
    val sut: SUT[Unit] = Action.void.publish(10)
    val sutError: SUT[Unit] =
      sut >> Action.reject("Some error").publish(20) >> Action
        .accept(10)
        .publish(10)

    val res = sut.run.value

    assertEquals(res.decision, Decision.InDecisive(()))
    assertEquals(res.notifications, Seq(10L))

    val res2 = sut.publish(20).run.value

    assertEquals(res2.decision, Decision.InDecisive(()))
    assertEquals(res2.notifications, Seq(10L, 20L))

    val res3 = sut.reset.run.value

    assertEquals(res3.decision, Decision.InDecisive(()))
    assert(res3.notifications.isEmpty, "Non empty notifications!")

    val res4 = sutError.run.value

    assertEquals(res4.decision, Decision.Rejected(NonEmptyChain("Some error")))
    assertEquals(res4.notifications, Seq(20L))
  }

  test("Decision") {
    val sut: SUT[Int] = Action.pure(100).publish(10)

    val errorSut: SUT[Int] = for {
      i <- sut
      _ <- Action.reject("Some error")
    } yield i

    def ac1(i: Int): SUT[Unit] = Action.accept(i + 1, i + 2)
    def ac2(i: Int): SUT[Unit] = Action.accept(i + 3)
    val acceptSut: SUT[Int] = for {
      i <- sut
      _ <- ac1(i)
      _ <- ac2(i)
    } yield i * 2

    val res = sut.run.value

    assertEquals(res.decision, Decision.InDecisive(100))
    assertEquals(res.notifications, Seq(10L))

    val errorSut2: SUT[Int] = acceptSut >> Action.reject("Some error")
    val res2 = errorSut.run.value
    val res3 = errorSut2.run.value

    assertEquals(res2, res3)
    assertEquals(res2.decision, Decision.Rejected(NonEmptyChain("Some error")))
    assertEquals(res2.notifications, Nil)

    val res4 = acceptSut.run.value

    assertEquals(
      res4.decision,
      Decision.Accepted(NonEmptyChain(101, 102, 103), 200)
    )
    assertEquals(res4.notifications, Seq(10L))
  }

  property("Accumulates notifications on accepted") {
    forAll(accepted, accepted) { (a, b) =>
      val c = a >> b

      val ares = a.run.value
      val bres = b.run.value
      val cres = c.run.value

      assertEquals(
        cres.decision,
        ares.decision >> bres.decision
      ) // just for doc
      assertEquals(cres.notifications, ares.notifications ++ bres.notifications)
    }
  }
  property("Rejected terminates") {
    forAll(rejected, notRejected) { (a, b) =>
      val c = a >> b

      val ares = a.run.value
      val bres = b.run.value
      val cres = c.run.value

      assertEquals(cres.decision, ares.decision) // just for doc
      assertEquals(cres.notifications, ares.notifications)
    }
  }
  property("adds notification to rejected action") {
    forAll(rejected, notifications) { (a, ns) =>
      val c = a.publish(ns: _*)

      val ares = a.run.value
      val cres = c.run.value

      assertEquals(cres.decision, ares.decision)
      assertEquals(cres.notifications, ares.notifications ++ ns)
    }
  }
  property("Reset will clear notifications") {
    forAll(anyAction) { a =>
      val b = a.reset

      val ares = a.run.value
      val bres = b.run.value

      assertEquals(bres.decision, ares.decision) // just for doc
      assertEquals(bres.notifications, Nil)
    }
  }
}

object ActionTest {
  type Rejection = String
  type Event = Int
  type Notification = Long

  type SUT[T] = Action[Eval, Rejection, Event, Notification, T]
  type SUT2 = SUT[Long]

  private val notification: Gen[Notification] = Arbitrary.arbitrary[Long]
  private val notifications: Gen[Seq[Notification]] =
    Gen.containerOf[Seq, Notification](notification)

  private def actionFor(g: Gen[Decision[Rejection, Event, Long]]): Gen[SUT2] =
    for {
      dc <- g
      ns <- notifications
    } yield Action.liftD(dc).publish(ns: _*)

  val accepted: Gen[SUT2] = actionFor(Generators.accepted)
  val rejected: Gen[SUT2] = actionFor(Generators.rejected)
  val indecisive: Gen[SUT2] = actionFor(Generators.indecisive)
  val notRejected: Gen[SUT2] = Gen.oneOf(accepted, indecisive)
  val anyAction: Gen[SUT2] = Gen.oneOf(accepted, rejected, indecisive)
}
