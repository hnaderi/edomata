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

import cats.data.NonEmptyChain
import cats.implicits.*
import edomata.core.Decision.Accepted
import edomata.core.Decision.InDecisive
import edomata.core.Decision.Rejected
import edomata.syntax.all.*
import munit.FunSuite

class DecisionSyntaxSuite extends FunSuite {
  test("Decision.apply") {
    assertEquals(Decision(1), InDecisive(1))
  }

  test("Decision.accept") {
    assertEquals(Decision.accept(1, 2, 3), Accepted(NonEmptyChain(1, 2, 3), ()))
  }

  test("Decision.acceptReturn") {
    assertEquals(
      Decision.acceptReturn(0)(1, 2, 3),
      Accepted(NonEmptyChain(1, 2, 3), 0)
    )
  }

  test("Decision.reject") {
    assertEquals(Decision.reject(1, 2, 3), Rejected(NonEmptyChain(1, 2, 3)))
  }

  test("value .accept") {
    assertEquals(1.accept, Decision.accept(1))
  }

  test("value .reject") {
    assertEquals(1.reject, Decision.reject(1))
  }

  test("value .asDecision") {
    assertEquals(1.asDecision, Decision(1))
  }

  test("Option .toDecision") {
    assertEquals(1.some.toDecision(0), Decision(1))
    assertEquals(None.toDecision(0), Decision.reject(0))
    assertEquals(None.toDecision(0, 1), Decision.reject(0, 1))
  }

  test("Option .toAccepted") {
    assertEquals(1.some.toAccepted, Decision.accept(1))
    assertEquals(None.toAccepted, Decision.unit)
  }

  test("Option .toAcceptedOr") {
    assertEquals(1.some.toAcceptedOr(2, 3), Decision.accept(1))
    assertEquals(None.toAcceptedOr(2, 3), Decision.reject(2, 3))
  }

  test("Option .toRejected") {
    assertEquals(1.some.toRejected, Decision.reject(1))
    assertEquals(None.toRejected, Decision.unit)
  }

  test("Either .toDecision") {
    assertEquals(1.asRight.toDecision, Decision(1))
    assertEquals(1.asLeft.toDecision, Decision.reject(1))
  }

  test("Either .toAccepted") {
    assertEquals(1.asRight.toAccepted, Decision.accept(1))
    assertEquals(1.asLeft.toAccepted, Decision.reject(1))
  }

  test("EitherNec .toDecision") {
    assertEquals(1.rightNec.toDecision, Decision(1))
    assertEquals(
      NonEmptyChain(1, 2, 3).asLeft.toDecision,
      Decision.reject(1, 2, 3)
    )
  }

  test("EitherNec .toAccepted") {
    assertEquals(1.rightNec.toAccepted, Decision.accept(1))
    assertEquals(
      NonEmptyChain(1, 2, 3).asLeft.toAccepted,
      Decision.reject(1, 2, 3)
    )
  }

  test("Decision .validate Either") {
    val validate = (i: Int) => if i > 0 then i.asRight else i.asLeft
    assertEquals(Decision(0).validate(validate), Decision.reject(0))
    assertEquals(Decision(1).validate(validate), Decision(1))
  }

  test("Decision .validate EitherNec") {
    val validate =
      (i: Int) => if i > 0 then i.rightNec else NonEmptyChain(i, i).asLeft
    assertEquals(Decision(0).validate(validate), Decision.reject(0, 0))
    assertEquals(Decision(1).validate(validate), Decision(1))
  }

  test("Decision .validate ValidatedNec") {
    val validate =
      (i: Int) => if i > 0 then i.validNec else NonEmptyChain(i, i).invalid
    assertEquals(Decision(0).validate(validate), Decision.reject(0, 0))
    assertEquals(Decision(1).validate(validate), Decision(1))
  }

  test("Decision.acceptWhen(boolean)(evs)") {
    assertEquals(Decision.acceptWhen(false)(1, 2, 3), Decision.unit)
    assertEquals(Decision.acceptWhen(true)(1, 2, 3), Decision.accept(1, 2, 3))
  }

  test("Decision.acceptWhen(option)") {
    assertEquals(Decision.acceptWhen(1.some), Decision.accept(1))
    assertEquals(Decision.acceptWhen(None), Decision.unit)
    assertEquals(Decision.acceptWhen(1.some)(2, 3), Decision.accept(1))
    assertEquals(Decision.acceptWhen(None)(2, 3), Decision.reject(2, 3))
  }

  test("Decision.acceptWhen(either)") {
    assertEquals(Decision.acceptWhen(1.asRight), Decision.accept(1))
    assertEquals(Decision.acceptWhen(1.asLeft), Decision.reject(1))
  }

  test("Decision.acceptWhen(eitherNec)") {
    assertEquals(Decision.acceptWhen(1.rightNec), Decision.accept(1))
    assertEquals(
      Decision.acceptWhen(NonEmptyChain(2, 3).asLeft),
      Decision.reject(2, 3)
    )
  }

  test("Decision.fromOption(option)") {
    assertEquals(Decision.fromOption(1.some, 2), Decision(1))
    assertEquals(Decision.fromOption(None, 2, 3), Decision.reject(2, 3))
  }

  test("Decision.fromEither(either)") {
    assertEquals(Decision.fromEither(1.asRight), Decision(1))
    assertEquals(Decision.acceptWhen(1.asLeft), Decision.reject(1))
  }

  test("Decision.fromEitherNec(eitherNec)") {
    assertEquals(Decision.fromEitherNec(1.rightNec), Decision(1))
    assertEquals(
      Decision.fromEitherNec(NonEmptyChain(2, 3).asLeft),
      Decision.reject(2, 3)
    )
  }

  test("Decision.validate(validatedNec)") {
    assertEquals(Decision.validate(1.validNec), Decision(1))
    assertEquals(
      Decision.validate(NonEmptyChain(2, 3).invalid),
      Decision.reject(2, 3)
    )
  }
}
