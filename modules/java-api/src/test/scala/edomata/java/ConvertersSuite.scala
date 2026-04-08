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

import cats.data.Chain
import cats.data.NonEmptyChain
import cats.implicits.*
import edomata.core.Decision
import munit.FunSuite

class ConvertersSuite extends FunSuite {

  test("toJavaList converts Chain to Java List") {
    val chain = Chain("a", "b", "c")
    val jList = Converters.toJavaList(chain)
    assertEquals(jList.size(), 3)
    assertEquals(jList.get(0), "a")
    assertEquals(jList.get(2), "c")
  }

  test("toJavaList converts NonEmptyChain to Java List") {
    val nec = NonEmptyChain("x", "y")
    val jList = Converters.toJavaList(nec)
    assertEquals(jList.size(), 2)
    assertEquals(jList.get(0), "x")
  }

  test("toChain converts Java List to Chain") {
    val jList = java.util.Arrays.asList("a", "b")
    val chain = Converters.toChain(jList)
    assertEquals(chain.toList, List("a", "b"))
  }

  test("toNonEmptyChain converts non-empty Java List") {
    val jList = java.util.Arrays.asList(1, 2, 3)
    val result = Converters.toNonEmptyChain(jList)
    assert(result.isDefined)
    assertEquals(result.get.toList, List(1, 2, 3))
  }

  test("toNonEmptyChain returns None for empty Java List") {
    val jList = java.util.Collections.emptyList[Int]()
    val result = Converters.toNonEmptyChain(jList)
    assert(result.isEmpty)
  }

  test("Decision.Accepted round-trips through converters") {
    val original = Decision.Accepted(NonEmptyChain("e1", "e2"), 42)
    val jDecision = Converters.decisionToJava(original)
    assert(jDecision.isAccepted)
    val backToScala = Converters.decisionToScala(jDecision)
    assertEquals(backToScala, original)
  }

  test("Decision.Rejected round-trips through converters") {
    val original: Decision[String, Nothing, Nothing] =
      Decision.Rejected(NonEmptyChain("r1"))
    val jDecision = Converters.decisionToJava(original)
    assert(jDecision.isRejected)
    val backToScala = Converters.decisionToScala(jDecision)
    assertEquals(backToScala, original)
  }

  test("Decision.InDecisive round-trips through converters") {
    val original: Decision[Nothing, Nothing, String] = Decision.InDecisive("ok")
    val jDecision = Converters.decisionToJava(original)
    assert(jDecision.isIndecisive)
    val backToScala = Converters.decisionToScala(jDecision)
    assertEquals(backToScala, original)
  }
}
