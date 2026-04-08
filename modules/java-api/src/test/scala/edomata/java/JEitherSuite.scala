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

import munit.FunSuite

class JEitherSuite extends FunSuite {

  test("left creates Left") {
    val e = JEither.left("error")
    assert(e.isLeft)
    assert(!e.isRight)
    assertEquals(e.getLeft, "error")
  }

  test("right creates Right") {
    val e = JEither.right(42)
    assert(e.isRight)
    assert(!e.isLeft)
    assertEquals(e.getRight, 42)
  }

  test("getRight on Left throws") {
    val e = JEither.left("error")
    intercept[java.util.NoSuchElementException](e.getRight)
  }

  test("getLeft on Right throws") {
    val e = JEither.right(42)
    intercept[java.util.NoSuchElementException](e.getLeft)
  }

  test("fold applies correct function") {
    val left: JEither[String, Int] = JEither.left("err")
    val right: JEither[String, Int] = JEither.right(10)

    assertEquals(
      left.fold[String, Int, String](l => s"L:$l", r => s"R:$r"),
      "L:err"
    )
    assertEquals(
      right.fold[String, Int, String](l => s"L:$l", r => s"R:$r"),
      "R:10"
    )
  }

  test("map transforms Right value") {
    val e: JEither[String, Int] = JEither.right(5)
    val mapped = e.map[Int, String](x => s"v=$x")
    assertEquals(mapped.getRight, "v=5")
  }

  test("map on Left is identity") {
    val e: JEither[String, Int] = JEither.left("err")
    val mapped = e.map[Int, String](x => s"v=$x")
    assert(mapped.isLeft)
    assertEquals(mapped.getLeft, "err")
  }
}
