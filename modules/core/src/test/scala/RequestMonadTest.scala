package edomata.core

import cats.Applicative
import cats.Eval
import cats.data.NonEmptyChain
import cats.implicits.*
import munit.*
import org.scalacheck.Arbitrary
import org.scalacheck.Arbitrary.arbitrary
import org.scalacheck.Gen
import org.scalacheck.Prop.forAll

import RequestMonadTest.*

class RequestMonadTest extends ScalaCheckSuite {
  property("Accumulates notifications") {
    forAll(input, program, program) { (inp, p1, p2) =>
      val p3 = p1 >> p2

      val n1 = p1.run(inp).value.notifications
      val n2 = p2.run(inp).value.notifications
      val n3 = p3.run(inp).value.notifications

      assertEquals(n3, n1 ++ n2)
    }
  }
}

object RequestMonadTest {
  type SUT = RequestMonad[Eval, Int, String, Long]

  val program: Gen[SUT] = for {
    res <- arbitrary[Long]
    notifs <- Gen.containerOf[Seq, String](arbitrary[String])
  } yield RequestMonad.lift(Response(res, notifs))

  val input: Gen[Int] = arbitrary[Int]

}
