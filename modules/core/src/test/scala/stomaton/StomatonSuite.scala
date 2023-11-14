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
package stomaton

import cats.data.NonEmptyChain
import cats.implicits.*
import cats.kernel.laws.discipline.EqTests
import cats.laws.discipline.*
import cats.laws.discipline.arbitrary.*
import cats.laws.discipline.eq.*
import munit.*
import org.scalacheck.Arbitrary
import org.scalacheck.Gen
import edomata.core.*

type Rejection = String
type Event = Int
type State = Int
type AppS[Env, T] = Stomaton[Option, Env, State, Rejection, Event, T]
type App[T] = AppS[Int, T]
type AppContra[T] = AppS[T, Unit]

final case class Notification(value: String = "")

class StomatonSuite extends DisciplineSuite {

  private given [Env, T: Arbitrary]: Arbitrary[AppS[Env, T]] = Arbitrary(
    for {
      d <- Gen.either(
        Arbitrary.arbitrary[NonEmptyChain[Rejection]],
        Arbitrary.arbitrary[T]
      )
    } yield Stomaton.decide(d)
  )

  private given ExhaustiveCheck[Int] = ExhaustiveCheck.instance(List(1, 2, 3))

  checkAll(
    "laws",
    MonadErrorTests[App, NonEmptyChain[String]].monadError[Int, Int, String]
  )
  checkAll(
    "laws",
    ContravariantTests[AppContra].contravariant[Int, Long, Boolean]
  )
  checkAll("laws", EqTests[App[Long]].eqv)
}
