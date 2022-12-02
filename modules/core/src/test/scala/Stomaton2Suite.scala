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

import cats.Eval
import cats.Monad
import cats.data.NonEmptyChain
import cats.implicits.*
import cats.kernel.Eq
import cats.kernel.laws.discipline.EqTests
import cats.laws.discipline.*
import cats.laws.discipline.arbitrary.*
import cats.laws.discipline.eq.*
import munit.*
import org.scalacheck.Arbitrary
import org.scalacheck.Gen
import org.scalacheck.Prop.forAll

class Stomaton2Suite extends DisciplineSuite {
  type State = Int
  type AppS[Env, T] = Stomaton2[Option, Env, State, Rejection, Event, T]
  type App[T] = AppS[Int, T]
  type AppContra[T] = AppS[T, Unit]

  private given [Env, T: Arbitrary]: Arbitrary[AppS[Env, T]] = Arbitrary(
    for {
      d <- Gen.either(
        Arbitrary.arbitrary[NonEmptyChain[Rejection]],
        Arbitrary.arbitrary[T]
      )
    } yield Stomaton2.decide(d)
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

  // test("Sanity") {
  //   val a: App[String] = Stomaton.pure("Hello")
  //   val ss: App[Int] = Stomaton.modify(_ * 2)

  //   val b: App[Int] = for {
  //     s <- a.set(24)
  //     st <- Stomaton.state
  //     // _ <- Stomaton.set(10)
  //     // _ <- Stomaton.reject("error")
  //     // s2 <- Stomaton.modify(_ * 2)
  //     ctx <- Stomaton.context
  //     _ <- ss
  //   } yield ctx

  //   println(b.runF(1, 100))
  // }
}
