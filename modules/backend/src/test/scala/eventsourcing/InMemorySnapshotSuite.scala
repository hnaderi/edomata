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
package eventsourcing

import cats.effect.IO
import cats.implicits.*
import munit.CatsEffectSuite

import InMemorySnapshotStoreSuite.*

class InMemorySnapshotStoreSuite extends CatsEffectSuite {
  test("Must not store more than size") {
    for {
      sut <- SnapshotStore.inMem[IO, String](1)
      _ <- sut.put("a", aggregate("va", 1))
      _ <- sut.get("a").assertEquals(aggregate("va", 1).some)
      _ <- sut.put("b", aggregate("vb", 3))
      _ <- sut.get("b").assertEquals(aggregate("vb", 3).some)
      _ <- sut.get("a").assertEquals(None)
    } yield ()
  }

  test("Must be convergent") {
    for {
      sut <- SnapshotStore.inMem[IO, String](1)
      _ <- sut.put("a", aggregate("vvv", 3))
      _ <- sut.put("a", aggregate("vv", 2))
      _ <- sut.get("a").assertEquals(aggregate("vvv", 3).some)
    } yield ()
  }

}

object InMemorySnapshotStoreSuite {
  private def aggregate(s: String, v: Long): AggregateState.Valid[String] =
    AggregateState.Valid(s, v)
}
