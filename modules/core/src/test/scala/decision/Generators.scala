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

import org.scalacheck.Arbitrary
import org.scalacheck.Gen
import edomata.core.Decision

object Generators {
  val accepted: Gen[Decision.Accepted[Event, Long]] = for {
    v <- Arbitrary.arbitrary[Long]
    l <- necOf(Arbitrary.arbitrary[Int])
  } yield Decision.Accepted(l, v)

  val rejected: Gen[Decision.Rejected[Rejection]] =
    necOf(Arbitrary.arbitrary[String]).map(Decision.Rejected(_))

  val indecisive: Gen[Decision.InDecisive[Long]] =
    Arbitrary.arbitrary[Long].map(Decision.InDecisive(_))

  val anySut: Gen[SUT] = Gen.oneOf(accepted, rejected, indecisive)

  private def ind[T: Arbitrary] = Arbitrary.arbitrary[T].map(Decision.pure(_))
  def decision[T: Arbitrary]: Gen[Dec[T]] = Gen.oneOf(
    for {
      a <- accepted
      d <- ind[T]
    } yield a >> d,
    ind[T],
    rejected
  )

  val notRejected: Gen[SUT] = Gen.oneOf(accepted, indecisive)
}
