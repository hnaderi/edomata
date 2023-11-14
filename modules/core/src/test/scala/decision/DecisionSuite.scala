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

package tests
package decision

import cats.data.*
import cats.implicits.*
import cats.kernel.laws.discipline.EqTests
import cats.kernel.laws.discipline.SerializableTests
import cats.laws.discipline.MonadErrorTests
import cats.laws.discipline.TraverseTests
import cats.laws.discipline.arbitrary.catsLawsCogenForNonEmptyChain
import munit.*
import org.scalacheck.Arbitrary
import org.scalacheck.Gen
import org.scalacheck.Prop.forAll
import edomata.core.Decision

import Generators.*

type Dec[T] = Decision[String, Int, T]
type Rejection = String
type Event = Int
type SUT = Dec[Long]

class DecisionSuite extends DisciplineSuite {
  private given [T: Arbitrary]: Arbitrary[Dec[T]] = Arbitrary(
    Arbitrary.arbitrary[T].flatMap(t => Generators.anySut.map(_.as(t)))
  )
  private given Arbitrary[NonEmptyChain[String]] = Arbitrary(
    necOf(Arbitrary.arbitrary[String])
  )

  checkAll(
    "laws",
    MonadErrorTests[Dec, NonEmptyChain[String]].monadError[Int, Int, String]
  )
  checkAll(
    "laws",
    TraverseTests[Dec].traverse[Int, Int, Int, Set[Int], Option, Option]
  )
  checkAll("laws", EqTests[Dec[Long]].eqv)
  checkAll("laws", SerializableTests.serializable[SUT](Decision.pure(123L)))

  property("Accepted accumulates") {
    forAll(accepted, accepted) { (a, b) =>
      val c = a.flatMap(_ => b)

      assertEquals(c, Decision.Accepted(a.events ++ b.events, b.result))
    }
  }
  property("Rejected terminates") {
    forAll(notRejected, rejected) { (a, b) =>
      val c = a.flatMap(_ => b)

      assertEquals(c, b)
    }
  }
  property("Rejected does not change") {
    forAll(rejected, notRejected) { (a, b) =>
      val c = a.flatMap(_ => b)

      assertEquals(c, a)
    }
  }

  private val anyValidationEN: Gen[Long => EitherNec[String, Long]] =
    Gen.function1(anySut.map(_.toEither))
  private val anyValidationE: Gen[Long => Either[String, Long]] =
    anyValidationEN.map(_.andThen(_.leftMap(_.head)))
  private val anyValidationV: Gen[Long => ValidatedNec[String, Long]] =
    Gen.function1(anySut.map(_.toValidated))

  property("validation using ValidatedNec") {
    forAll(anySut, anyValidationV) { (a, f) =>
      assertEquals(a.validate(f), a.flatMap(v => Decision.validate(f(v))))
    }
  }

  property("validation using Either") {
    forAll(anySut, anyValidationE) { (a, f) =>
      assertEquals(a.validate(f), a.flatMap(v => Decision.fromEither(f(v))))
    }
  }

  property("validation using EitherNec") {
    forAll(anySut, anyValidationEN) { (a, f) =>
      assertEquals(a.validate(f), a.flatMap(v => Decision.fromEitherNec(f(v))))
    }
  }

  property("assertion using ValidatedNec") {
    forAll(anySut, anyValidationV) { (a, f) =>
      assertEquals(a.assert(f), a.flatTap(v => Decision.validate(f(v))))
    }
  }

  property("assertion using Either") {
    forAll(anySut, anyValidationE) { (a, f) =>
      assertEquals(a.assert(f), a.flatTap(v => Decision.fromEither(f(v))))
    }
  }

  property("assertion using EitherNec") {
    forAll(anySut, anyValidationEN) { (a, f) =>
      assertEquals(a.assert(f), a.flatTap(v => Decision.fromEitherNec(f(v))))
    }
  }
}
