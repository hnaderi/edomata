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
package cqrs

import cats.effect.IO
import cats.effect.testkit.TestControl
import fs2.Chunk
import fs2.Stream
import munit.CatsEffectSuite

import scala.concurrent.duration.*

class NotificationsSuite extends CatsEffectSuite {
  test("Must notify outbox listeners") {
    TestControl.executeEmbed(
      for {
        ns <- Notifications[IO]
        _ <- ns.notifyOutbox
        _ <- assertNotified(ns.outbox)
      } yield ()
    )
  }

  test("Must notify outbox listeners once") {
    TestControl.executeEmbed(
      for {
        ns <- Notifications[IO]
        _ <- ns.notifyOutbox
        _ <- ns.notifyOutbox
        _ <- ns.notifyOutbox
        _ <- assertNotified(ns.outbox)
      } yield ()
    )
  }

  test("Must notify state listeners") {
    TestControl.executeEmbed(
      for {
        ns <- Notifications[IO]
        _ <- ns.notifyState
        _ <- assertNotified(ns.state)
      } yield ()
    )
  }

  test("Must notify state listeners once") {
    TestControl.executeEmbed(
      for {
        ns <- Notifications[IO]
        _ <- ns.notifyState
        _ <- ns.notifyState
        _ <- ns.notifyState
        _ <- assertNotified(ns.state)
      } yield ()
    )
  }
  private def assertNotified(s: Stream[IO, Unit]) =
    s.groupWithin(10, 10.hours)
      .head
      .compile
      .lastOrError
      .assertEquals(Chunk(()))
}
