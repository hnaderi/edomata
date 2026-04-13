/*
 * Copyright 2021 Beyond Scale Group
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

package edomata.java

import cats.data.NonEmptyChain
import cats.implicits.*
import edomata.core.Decision
import munit.FunSuite

class JDomainModelSuite extends FunSuite {

  // Simple counter domain for testing
  val counterModel: JDomainModel[Int, String, String] =
    new JDomainModel[Int, String, String] {
      def initial: Int = 0
      def transition(
          event: String,
          state: Int
      ): JEither[java.util.List[String], Int] =
        event match {
          case s if s.startsWith("+") =>
            JEither.right(state + s.substring(1).toInt)
          case s if s.startsWith("-") =>
            val amount = s.substring(1).toInt
            if (state >= amount) JEither.right(state - amount)
            else JEither.left(java.util.Arrays.asList("insufficient balance"))
          case _ =>
            JEither.left(java.util.Arrays.asList("unknown event"))
        }
    }

  test("initial state is correct") {
    assertEquals(counterModel.initial, 0)
  }

  test("transition returns Right for valid event") {
    val result = counterModel.transition("+5", 10)
    assert(result.isRight)
    assertEquals(result.getRight, 15)
  }

  test("transition returns Left for invalid event") {
    val result = counterModel.transition("-20", 10)
    assert(result.isLeft)
  }

  test("toModelTC produces valid ModelTC") {
    val modelTC = counterModel.toModelTC
    assertEquals(modelTC.initial, 0)

    // Test that transition works through ModelTC
    val transResult = modelTC.transition("+3")(7)
    assert(transResult.isValid)
    assertEquals(transResult.toOption.get, 10)
  }

  test("toModelTC transition rejects correctly") {
    val modelTC = counterModel.toModelTC
    val transResult = modelTC.transition("-100")(5)
    assert(transResult.isInvalid)
  }

  test("toModelTC performs decisions correctly") {
    val modelTC = counterModel.toModelTC
    val decision = Decision.accept("+10")
    val result = modelTC.perform(0, decision)
    result match {
      case Decision.Accepted(_, newState) =>
        assertEquals(newState, 10)
      case _ => fail("Expected Accepted")
    }
  }

  test("JDomainModel.create factory works") {
    val model = JDomainModel.create[Int, String, String](
      0,
      (event, state) => JEither.right(state + event.length)
    )
    assertEquals(model.initial, 0)
    assertEquals(model.transition("hello", 5).getRight, 10)
  }
}
