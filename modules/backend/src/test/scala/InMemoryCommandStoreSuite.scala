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

package edomata.backend

import cats.effect.IO
import edomata.core.*
import munit.CatsEffectSuite

import java.time.Instant

import InMemoryCommandStoreSuite.*

class InMemoryCommandStoreSuite extends CatsEffectSuite {

  test("Must add") {
    for {
      ics <- CommandStore.inMem[IO](1)
      _ <- ics.append(cmd("a"))
      _ <- ics.contains("a").assertEquals(true)
    } yield ()
  }

  test("Must remove when cache size is reached") {
    for {
      ics <- CommandStore.inMem[IO](1)
      _ <- ics.append(cmd("a"))
      _ <- ics.append(cmd("b"))
      _ <- ics.contains("a").assertEquals(false)
      _ <- ics.contains("b").assertEquals(true)
    } yield ()
  }
}
object InMemoryCommandStoreSuite {
  def cmd(i: String) = CommandMessage(i, Instant.MAX, "", ())
}
