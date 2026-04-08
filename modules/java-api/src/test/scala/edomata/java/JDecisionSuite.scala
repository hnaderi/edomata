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

class JDecisionSuite extends FunSuite {

  test("accept creates Accepted with events") {
    val d = JDecision.accept[String, String]("e1", "e2")
    assert(d.isAccepted)
    assert(!d.isRejected)
    assert(!d.isIndecisive)
    d match {
      case JDecision.Accepted(events, result) =>
        assertEquals(events.size(), 2)
        assertEquals(events.get(0), "e1")
        assertEquals(events.get(1), "e2")
        assertEquals(result, ())
      case _ => fail("Expected Accepted")
    }
  }

  test("acceptReturn creates Accepted with value and events") {
    val d = JDecision.acceptReturn[String, String, Int](42, "e1")
    assert(d.isAccepted)
    d match {
      case JDecision.Accepted(events, result) =>
        assertEquals(events.size(), 1)
        assertEquals(result, 42)
      case _ => fail("Expected Accepted")
    }
  }

  test("reject creates Rejected with reasons") {
    val d = JDecision.reject[String, String]("r1", "r2", "r3")
    assert(d.isRejected)
    assert(!d.isAccepted)
    d match {
      case JDecision.Rejected(reasons) =>
        assertEquals(reasons.size(), 3)
        assertEquals(reasons.get(0), "r1")
      case _ => fail("Expected Rejected")
    }
  }

  test("pure creates Indecisive") {
    val d = JDecision.pure[String, String, String]("hello")
    assert(d.isIndecisive)
    assert(!d.isAccepted)
    assert(!d.isRejected)
    d match {
      case JDecision.Indecisive(result) =>
        assertEquals(result, "hello")
      case _ => fail("Expected Indecisive")
    }
  }

  test("unit is Indecisive with Unit") {
    assert(JDecision.unit[String, String].isIndecisive)
  }

  test("map transforms the result value") {
    val d = JDecision.pure[String, String, Int](10)
    val mapped = d.map[Int, String](x => s"v=$x")
    mapped match {
      case JDecision.Indecisive(result) =>
        assertEquals(result, "v=10")
      case _ => fail("Expected Indecisive")
    }
  }

  test("map on Accepted preserves events") {
    val d = JDecision.acceptReturn[String, String, Int](5, "e1")
    val mapped = d.map[Int, String](x => s"n=$x")
    mapped match {
      case JDecision.Accepted(events, result) =>
        assertEquals(events.size(), 1)
        assertEquals(result, "n=5")
      case _ => fail("Expected Accepted")
    }
  }

  test("map on Rejected is a no-op") {
    val d = JDecision.reject[String, String]("fail")
    val mapped = d.map[Unit, String](_ => "never")
    assert(mapped.isRejected)
  }

  test("flatMap chains Accepted decisions, merging events") {
    val d1 = JDecision.accept[String, String]("e1")
    val d2 = d1.flatMap[String, String, Unit, Unit](_ =>
      JDecision.accept[String, String]("e2")
    )
    d2 match {
      case JDecision.Accepted(events, _) =>
        assertEquals(events.size(), 2)
        assertEquals(events.get(0), "e1")
        assertEquals(events.get(1), "e2")
      case _ => fail("Expected Accepted")
    }
  }

  test("flatMap short-circuits on Rejected") {
    val d1 = JDecision.reject[String, String]("r1")
    val d2 = d1.flatMap[String, String, Unit, Unit](_ =>
      JDecision.pure[String, String, Unit](())
    )
    assert(d2.isRejected)
  }

  test("flatMap Accepted then Rejected yields Rejected") {
    val d = JDecision.accept[String, String]("e1")
    val result = d.flatMap[String, String, Unit, Unit](_ =>
      JDecision.reject[String, String]("r1")
    )
    assert(result.isRejected)
  }

  test("flatMap Indecisive delegates to function result") {
    val d = JDecision.pure[String, String, Int](1)
    val result = d.flatMap[String, String, Int, Unit](_ =>
      JDecision.accept[String, String]("e1")
    )
    assert(result.isAccepted)
  }

  test("toEither returns Right for Accepted") {
    val d = JDecision.acceptReturn[String, String, String]("ok", "e1")
    val e = d.toEither
    assert(e.isRight)
    assertEquals(e.getRight, "ok")
  }

  test("toEither returns Left for Rejected") {
    val d = JDecision.reject[String, String]("r1")
    val e = d.toEither
    assert(e.isLeft)
  }

  test("toEither returns Right for Indecisive") {
    val d = JDecision.pure[String, String, Int](42)
    val e = d.toEither
    assert(e.isRight)
    assertEquals(e.getRight, 42)
  }
}
