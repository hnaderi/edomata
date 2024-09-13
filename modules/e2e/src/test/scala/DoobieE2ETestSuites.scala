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
package doobie

import _root_.doobie.util.transactor.Transactor
import cats.effect.IO
import cats.effect.kernel.Resource
import dev.hnaderi.example.accounts.*
import edomata.backend.*
import edomata.doobie.*
import edomata.doobie.BackendCodec
import edomata.doobie.CirceCodec
import edomata.doobie.DoobieDriver
import io.circe.generic.auto.*

private given BackendCodec[Event] = CirceCodec.jsonb
private given BackendCodec[Notification] = CirceCodec.jsonb
private given BackendCodec[Account] = CirceCodec.jsonb

private def driver =
  Resource.eval(
    DoobieDriver(
      "doobie_e2e",
      Transactor
        .fromDriverManager[IO](
          driver = "org.postgresql.Driver",
          url = "jdbc:postgresql:postgres",
          user = "postgres",
          password = "postgres",
          logHandler = None
        )
    )
  )

class DoobieE2ETestSuites extends e2e(driver)
