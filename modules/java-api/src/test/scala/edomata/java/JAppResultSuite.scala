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

class JAppResultSuite extends FunSuite {

  test("decide creates result with decision and no notifications") {
    val result =
      JAppResult.decide[String, String, String](
        JDecision.accept[String, String]("e1")
      )
    assert(result.decision.isAccepted)
    assertEquals(result.notifications.size(), 0)
  }

  test("accept creates Accepted result") {
    val result = JAppResult.accept[String, String, String]("e1", "e2")
    assert(result.decision.isAccepted)
  }

  test("reject creates Rejected result") {
    val result = JAppResult.reject[String, String, String]("r1")
    assert(result.decision.isRejected)
  }

  test("publish creates Indecisive result with notifications") {
    val notifs = java.util.Arrays.asList("n1", "n2")
    val result = JAppResult.publish[String, String, String](notifs)
    assert(result.decision.isIndecisive)
    assertEquals(result.notifications.size(), 2)
    assertEquals(result.notifications.get(0), "n1")
  }

  test("decideAndPublish creates result with both") {
    val notifs = java.util.Arrays.asList("n1")
    val result = JAppResult.decideAndPublish[String, String, String](
      JDecision.accept[String, String]("e1"),
      notifs
    )
    assert(result.decision.isAccepted)
    assertEquals(result.notifications.size(), 1)
  }
}
