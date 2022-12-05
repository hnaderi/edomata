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

package tests.stomaton

import cats.*
import cats.data.*
import cats.implicits.*
import edomata.core.*
import munit.*

class StomatonConstructorSuite extends FunSuite {
  test("pure") {
    val a: App[String] = Stomaton.pure("hello")
    assertEquals(a.run(0, 0), Some(ResponseE(Right((0, "hello")))))
  }

  test("set state") {
    val a: App[Unit] = Stomaton.set(2)
    assertEquals(a.run(0, 0), Some(ResponseE(Right((2, ())))))
  }

  test("context") {
    val a: App[Int] = Stomaton.context
    assertEquals(a.run(10, 0), Some(ResponseE(Right((0, 10)))))
  }

  test("state") {
    val a: App[Int] = Stomaton.state
    assertEquals(a.run(10, 0), Some(ResponseE(Right((0, 0)))))
  }

  test("modify") {
    val a: App[Int] = Stomaton.modify(_ + 2)
    assertEquals(a.run(10, 0), Some(ResponseE(Right((2, 2)))))
  }

  test("modifyS") {
    val a: App[Int] = Stomaton.modifyS(i => Right(i + 2))
    assertEquals(a.run(10, 0), Some(ResponseE(Right((2, 2)))))

    val b: App[Int] = Stomaton.modifyS(i => "".leftNec)
    assertEquals(b.run(10, 0), Some(ResponseE(Left(NonEmptyChain("")))))
  }

  test("publish") {
    val a: App[Unit] = Stomaton.publish(1, 2, 3)
    assertEquals(a.run(10, 0), Some(ResponseE(Right((0, ())), Chain(1, 2, 3))))
  }
}
