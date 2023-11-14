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
import cats.effect.testkit.TestControl
import cats.implicits.*
import munit.CatsEffectSuite
import munit.Location

import scala.concurrent.duration.*

import PersistedSnapshotStoreSuite.*

class PersistedSnapshotStoreSuite extends CatsEffectSuite {
  check("Must not store more than size", size = 1) { sut =>
    sut.store.put("a", a) >>
      sut.assertPresent("a" -> a) >>
      sut.store.put("b", b) >>
      sut.assertNotPresent("a") >>
      sut.assertPresent("b" -> b)
  }

  check("Must be convergent", size = 1) { sut =>
    sut.store.put("a", aggregate("aa", 2)) >>
      sut.store.put("a", aggregate("a", 1)) >>
      sut.assertPresent("a" -> aggregate("aa", 2))
  }

  check(
    "Must persist evicted items asynchronously when maxWait reached",
    size = 1,
    maxWait = 1.minute
  ) { sut =>
    sut.store.put("a", a) >>
      sut.store.put("b", b) >>
      sut.assertNotPersisted("a", "b") >>
      IO.sleep(61.seconds) >>
      sut.assertPersisted("a" -> a) >>
      sut.assertNotPersisted("b")
  }

  check(
    "Must persist evicted items asynchronously when maxBuffer reached",
    size = 1,
    maxBuffer = 1
  ) { sut =>
    sut.store.put("a", a) >>
      sut.store.put("b", b) >>
      sut.assertNotPersisted("a", "b") >>
      IO.sleep(1.seconds) >>
      sut.assertPersisted("a" -> a) >>
      sut.assertNotPersisted("b")
  }

  check("Must not flush on exit if flushOnExit is false", flushOnExit = false)(
    sut =>
      sut.store.put("a", a) >>
        sut.store.put("b", b) >>
        sut.assertNotPersisted("a", "b"),
    p => p.get("a").assertEquals(None) >> p.get("b").assertEquals(None)
  )

  check("Must flush on exit if flushOnExit is true", flushOnExit = true)(
    sut =>
      sut.store.put("a", a) >>
        sut.store.put("b", b) >>
        sut.assertNotPersisted("a", "b"),
    p => p.get("a").assertEquals(Some(a)) >> p.get("b").assertEquals(Some(b))
  )

  check(
    "Must not persist anything if neither evicted or requested to flushOnExit",
    flushOnExit = false
  )(
    sut =>
      sut.store.put("a", a) >>
        sut.store.put("b", b),
    p => p.get("a").assertEquals(None) >> p.get("b").assertEquals(None)
  )

  check(
    "Must retry persisting",
    flushOnExit = false,
    size = 1,
    maxBuffer = 1,
    failures = 1
  )(sut =>
    sut.store.put("a", a) >>
      sut.store.put("b", b) >>
      IO.sleep(2.seconds) >>
      sut.assertPersisted("a" -> a)
  )

  final class TestUniverse(
      persistence: SnapshotPersistence[IO, String],
      val store: SnapshotStore[IO, String]
  ) {
    def assertNotPersisted(i: String*)(using Location) =
      i.traverse(persistence.get(_).assertEquals(None)).void

    def assertPersisted(i: (String, AggregateState.Valid[String])*)(using
        Location
    ) =
      i.traverse((k, v) => persistence.get(k).assertEquals(Some(v))).void

    def assertNotPresent(i: String*)(using Location) =
      i.traverse(store.get(_).assertEquals(None)).void

    def assertPresent(i: (String, AggregateState.Valid[String])*)(using
        Location
    ) =
      i.traverse((k, v) => store.get(k).assertEquals(Some(v))).void
  }

  def check(
      name: String,
      size: Int = 1000,
      maxBuffer: Int = 100,
      maxWait: FiniteDuration = 1.minute,
      flushOnExit: Boolean = true,
      failures: Int = 0
  )(
      f: TestUniverse => IO[Unit],
      g: SnapshotPersistence[IO, String] => IO[Unit] = _ => IO.unit
  )(using Location): Unit = test(name) {
    TestControl.executeEmbed(
      for {
        p <-
          if failures > 0 then FailingSnapshotPersistence[String](failures)
          else FakeSnapshotPersistence[String]
        _ <- SnapshotStore
          .persisted(
            p,
            size = size,
            maxBuffer = maxBuffer,
            maxWait = maxWait,
            flushOnExit = flushOnExit
          )
          .use(pss => f(TestUniverse(p, pss)))
        _ <- g(p)
      } yield ()
    )
  }

}

object PersistedSnapshotStoreSuite {
  private def aggregate(s: String, v: Long): AggregateState.Valid[String] =
    AggregateState.Valid(s, v)
  private val a = aggregate("a", 1)
  private val b = aggregate("b", 2)
}
