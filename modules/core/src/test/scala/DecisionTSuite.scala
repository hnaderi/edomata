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

import cats.Applicative
import cats.Eval
import cats.data.NonEmptyChain
import cats.implicits.*
import cats.kernel.laws.discipline.EqTests
import cats.laws.discipline.MonadTests
import munit.*
import org.scalacheck.Arbitrary
import org.scalacheck.Gen
import org.scalacheck.Prop.forAll

import DecisionTSuite.*

class DecisionTSuite extends DisciplineSuite {
  property("Accepted accumulates") {
    forAll(accepted, accepted) { case ((a, at), (b, bt)) =>
      val c = at.flatMap(_ => bt).run.value

      assertEquals(c, Decision.Accepted(a.events ++ b.events, b.result))
    }
  }
  property("Rejected terminates") {
    forAll(notRejected, rejected) { case ((a, at), (b, bt)) =>
      val c = at.flatMap(_ => bt).run.value

      assertEquals(c, b)
    }
  }
  property("Rejected does not change") {
    forAll(rejected, notRejected) { case ((a, at), (b, bt)) =>
      val c = at.flatMap(_ => bt).run.value

      assertEquals(c, a)
    }
  }

  checkAll("laws", MonadTests[DTT].monad[Int, Int, String])

  checkAll("laws", EqTests[DTT[Long]].eqv)

  private given [T: Arbitrary]: Arbitrary[DTT[T]] = Arbitrary(
    Arbitrary.arbitrary[T].flatMap(t => anySut2.map(_.as(t)))
  )
}

object DecisionTSuite {

  type SUTT = DecisionT[Eval, Rejection, Event, Long]

  type DTT[T] = DecisionT[Eval, Rejection, Event, T]

  private def lift[T <: SUT](t: Gen[T]): Gen[(T, SUTT)] =
    t.map(d => (d, DecisionT.lift(d)(using Applicative[Eval])))

  val accepted =
    lift(Generators.accepted)

  val rejected =
    lift(Generators.rejected)

  val indecisive =
    lift(Generators.indecisive)

  val anySut = Gen.oneOf(accepted, rejected, indecisive)
  val notRejected = Gen.oneOf(accepted, indecisive)

  val anySut2: Gen[DTT[Long]] =
    Gen.oneOf(accepted, rejected, indecisive).map(_._1).map(DecisionT.lift(_))
}
