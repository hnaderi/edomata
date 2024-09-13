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
package skunk

import _root_.skunk.Session
import cats.effect.IO
import dev.hnaderi.example.accounts.*
import edomata.skunk.BackendCodec
import edomata.skunk.CirceCodec
import edomata.skunk.SkunkDriver
import io.circe.generic.auto.*
import natchez.Trace.Implicits.noop

private given BackendCodec[Event] = CirceCodec.jsonb
private given BackendCodec[Notification] = CirceCodec.jsonb
private given BackendCodec[Account] = CirceCodec.jsonb

private def driver = Session
  .pooled[IO](
    host = "localhost",
    port = 5432,
    user = "postgres",
    password = Some("postgres"),
    database = "postgres",
    4
  )
  .evalMap(SkunkDriver("skunk_e2e", _))

class SkunkE2ETestSuites extends e2e(driver)
