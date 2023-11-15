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
package edomaton

import cats.*
import cats.data.*
import cats.implicits.*
import cats.kernel.laws.discipline.EqTests
import cats.laws.discipline.*
import cats.laws.discipline.eq.*
import munit.*
import org.scalacheck.Arbitrary
import org.scalacheck.Gen
import edomata.core.*
import tests.decision.Generators

type Dec[T] = Decision[String, Int, T]
type AppG[Env, T] =
  Edomaton[Option, Env, Rejection, Event, Notification, T]
type App[T] = AppG[Int, T]
type AppContra[T] = AppG[T, Unit]

type Rejection = String
type Event = Int
final case class Notification(value: String = "")

class EdomatonSuite extends DisciplineSuite {
  private val notifications: Gen[Chain[Notification]] =
    Gen
      .containerOf[Seq, Notification](
        Arbitrary.arbitrary[String].map(Notification(_))
      )
      .map(Chain.fromSeq)

  private given [Env, T: Arbitrary]: Arbitrary[AppG[Env, T]] = Arbitrary(
    for {
      n <- notifications
      t <- Arbitrary.arbitrary[T]
      d <- Generators.anySut
    } yield Edomaton(_ => Some(ResponseD(d.as(t), n)))
  )

  private given ExhaustiveCheck[Int] = ExhaustiveCheck.instance(List(1, 2, 3))

  checkAll(
    "laws",
    MonadTests[App].monad[Int, Long, String]
  )
  checkAll(
    "laws",
    ContravariantTests[AppContra].contravariant[Int, Long, Boolean]
  )
  checkAll("laws", EqTests[App[Long]].eqv)
}
