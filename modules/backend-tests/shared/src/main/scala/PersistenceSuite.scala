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

package tests

import cats.effect.IO
import cats.effect.kernel.Resource
import munit.CatsEffectSuite

abstract class PersistenceSuite(
    storage: Resource[IO, Storage[IO, String, Int, String, String]],
    name: String
) extends StorageSuite[String, Int, String, String](storage, name) {
  check("Must append correctly") { s =>
    for {
      _ <- s.repository.append(
        ctx = ???,
        version = ???,
        newState = ???,
        events = ???,
        notifications = ???
      )

      _ <- s.outbox.read.compile.toList.assertEquals(???)
      _ <- s.journal.readAll.compile.toList.assertEquals(???)
    } yield ()
  }

  check("Must notify correctly") { s =>
    for {
      _ <- s.repository.notify(
        ctx = ???,
        notifications = ???
      )

      _ <- s.outbox.read.compile.toList.assertEquals(???)
    } yield ()
  }
}
