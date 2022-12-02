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
import cats.Monoid
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

object StomatonExample {
  enum Foo {
    case Empty
    case Started(value: Int)

    def start(initial: Int) = this match {
      case Empty      => Decision.pure(Started(initial))
      case Started(_) => Decision.reject("Cannot start started foo!")
    }
  }

  object Foo extends CQRSModel[Foo, Int, String] {
    override def initial: Foo = Empty
  }

  object FooService extends Foo.Service[Int] {
    def apply(): PureApp[Unit] = Stomaton.unit

    import dsl.*
    def apply2(): PureApp[Foo] = for {
      _ <- pure(1)
      ns <- modifyS(_.start(1))
      _ <- Stomaton.unit
    } yield ns
    // def apply3(): PureApp[Foo] =
    //   pure(1).flatMap(modifyS(_.start(1)))
  }

  val res = FooService().runF(???, Foo.Empty)

  val out = res.visit(
    ???,
    (s, _) => s
  )
}
