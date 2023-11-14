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

package tests.responset

import cats.data.*
import cats.implicits.*
import cats.laws.discipline.TraverseTests
import cats.laws.discipline.arbitrary.catsLawsArbitraryForNonEmptyChain
import org.scalacheck.Arbitrary
import edomata.core.*

import tests.decision.Generators.*

type Dec[T] = Decision[String, Int, T]
type Rejection = String
type Event = Int
final case class Notification(value: String = "")

private given Arbitrary[Notification] = Arbitrary(
  Arbitrary.arbitrary[String].map(Notification(_))
)
class ResponseDecisionSuite
    extends ResponseTLaws[
      Decision[Rejection, Event, *],
      Rejection,
      Long,
      Notification
    ](
      rejected,
      notRejected
    ) {
  otherLaws(
    TraverseTests[App].traverse[Int, Int, Int, Set[Int], Option, Option]
  )
}

class ResponseEitherNecSuite
    extends ResponseTLaws[
      EitherNec[Rejection, *],
      Rejection,
      Long,
      Notification
    ](
      Arbitrary.arbitrary[NonEmptyChain[Rejection]].map(Left(_)),
      Arbitrary.arbitrary[Long].map(Right(_))
    ) {
  otherLaws(
    TraverseTests[App].traverse[Int, Int, Int, Set[Int], Option, Option]
  )
}
