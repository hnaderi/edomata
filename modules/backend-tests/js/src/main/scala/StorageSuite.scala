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
import edomata.backend.*
import munit.CatsEffectSuite
import munit.Location

abstract class StorageSuite[S, E, R, N](
    storage: Resource[IO, Storage[IO, S, E, R, N]],
    suiteName: String
) extends CatsEffectSuite {
  def check(name: String)(f: Storage[IO, S, E, R, N] => IO[Unit])(using
      Location
  ) = test(s"${suiteName}: ${name}")(storage.use(f))
}
