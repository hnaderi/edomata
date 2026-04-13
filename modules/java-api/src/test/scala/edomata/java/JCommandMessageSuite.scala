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

import java.time.Instant

class JCommandMessageSuite extends FunSuite {

  test("of creates command message with correct fields") {
    val time = Instant.parse("2024-01-01T00:00:00Z")
    val cmd = JCommandMessage.of("cmd-1", time, "aggregate-1", "payload")

    assertEquals(cmd.id, "cmd-1")
    assertEquals(cmd.time, time)
    assertEquals(cmd.address, "aggregate-1")
    assertEquals(cmd.payload, "payload")
  }

  test("underlying CommandMessage is correctly constructed") {
    val time = Instant.now()
    val cmd = JCommandMessage.of("id", time, "addr", 42)

    assertEquals(cmd.underlying.id, "id")
    assertEquals(cmd.underlying.address, "addr")
    assertEquals(cmd.underlying.payload, 42)
  }

  test("equality works based on underlying") {
    val time = Instant.parse("2024-06-15T12:00:00Z")
    val cmd1 = JCommandMessage.of("id", time, "addr", "p")
    val cmd2 = JCommandMessage.of("id", time, "addr", "p")
    assertEquals(cmd1, cmd2)
  }

  test("toString includes all fields") {
    val cmd = JCommandMessage.of("id", Instant.EPOCH, "addr", "data")
    val s = cmd.toString
    assert(s.contains("id"))
    assert(s.contains("addr"))
    assert(s.contains("data"))
  }
}
