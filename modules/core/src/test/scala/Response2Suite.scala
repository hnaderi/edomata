/*
 * Copyright 2021 Hossein Naderi
 *
 * Licensed under the Apache License, Version 2.0 (the "License");
 * you may not use this file except in compliance with the License.
 * You may obtain a copy of the License at
 *
 *     http://www.apache.org/licenses/LICENSE-2.0
 *
 * Unless required by applicable law or agreed to in writing, software
 * distributed under the License is distributed on an "AS IS" BASIS,
 * WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
 * See the License for the specific language governing permissions and
 * limitations under the License.
 */

package edomata.core

import cats.Monad
import cats.data.Chain
import cats.data.NonEmptyChain
import cats.implicits.*
import cats.kernel.laws.discipline.EqTests
import cats.laws.discipline.MonadTests
import cats.laws.discipline.TraverseTests
import cats.laws.discipline.arbitrary.catsLawsCogenForNonEmptyChain
import munit.*
import org.scalacheck.Arbitrary
import org.scalacheck.Gen
import org.scalacheck.Prop.forAll
import cats.laws.discipline.arbitrary.catsLawsArbitraryForNonEmptyChain
import Response2Suite.*
import Generators.*

class Response2Suite extends DisciplineSuite {

  type Res[T] = Response2[Rejection, Notification, T]
  private given [T: Arbitrary]: Arbitrary[Res[T]] = Arbitrary(
    for {
      n <- notifications
      d <- Gen.either(
        Arbitrary.arbitrary[NonEmptyChain[Rejection]],
        Arbitrary.arbitrary[T]
      )
    } yield Response2(d, n)
  )

  checkAll(
    "laws",
    MonadTests[Res].monad[Int, Int, String]
  )

  checkAll(
    "laws",
    TraverseTests[Res].traverse[Int, Int, Int, Set[Int], Option, Option]
  )

  checkAll("laws", EqTests[Res[Long]].eqv)

  // property("Accumulates on accept") {
  //   forAll(notRejected, notifications, notRejected, notifications) {
  //     (a1, n1, a2, n2) =>
  //       val r1 = Response(a1, n1)
  //       val r2 = Response(a2, n2)
  //       val r3 = r1 >> r2
  //       assertEquals(r3, Response(a1.flatMap(_ => a2), n1 ++ n2))
  //   }
  // }
  // property("Resets on rejection") {
  //   forAll(notRejected, notifications, rejected, notifications) {
  //     (a1, n1, a2, n2) =>
  //       val r1 = Response(a1, n1)
  //       val r2 = Response(a2, n2)
  //       val r3 = r1 >> r2
  //       assertEquals(r3, Response(a1.flatMap(_ => a2), n2))
  //       assert(r3.result.isRejected)
  //   }
  // }
  // property("Rejected does not change") {
  //   forAll(rejected, notifications, anySut, notifications) { (a1, n1, a2, n2) =>
  //     val r1 = Response(a1, n1)
  //     val r2 = Response(a2, n2)
  //     val r3 = r1 >> r2
  //     assertEquals(r3, r1)
  //     assert(r3.result.isRejected)
  //   }
  // }
  // property("Publish on rejection") {
  //   forAll(rejected, notifications, notifications) { (a1, n1, n2) =>
  //     val r1 = Response(a1, n1)
  //     val r2 = r1.publishOnRejection(n2.toList: _*)

  //     assertEquals(
  //       r2,
  //       r1.copy(notifications = r1.notifications ++ n2)
  //     )
  //   }
  // }
  // property("Publish adds notifications") {
  //   forAll(anySut, notifications, notifications) { (a1, n1, n2) =>
  //     val r1 = Response(a1, n1)
  //     val r2 = r1.publish(n2.toList: _*)
  //     assertEquals(r2, r1.copy(notifications = n1 ++ n2))
  //   }
  // }
  // property("Reset clears notifications") {
  //   forAll(anySut, notifications) { (a1, n1) =>
  //     val r1 = Response(a1, n1)
  //     val r2 = r1.reset
  //     assertEquals(r2.notifications, Chain.nil)
  //     assertEquals(r2.result, r1.result)
  //   }
  // }
}

object Response2Suite {
  val notifications: Gen[Chain[Notification]] =
    Gen
      .containerOf[Seq, Notification](
        Arbitrary.arbitrary[String].map(Notification(_))
      )
      .map(Chain.fromSeq)
}
