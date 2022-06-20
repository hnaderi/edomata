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
package doobie

import cats.MonadError
import cats.implicits.*

extension [F[_]](self: F[Int])(using F: MonadError[F, Throwable]) {
  private[backend] def assertInserted(size: Int): F[Unit] = self.flatMap { i =>
    if i == size then F.unit
    else
      F.raiseError(
        BackendError.PersistenceError(
          s"expected to insert exactly $size, but inserted $i"
        )
      )
  }
  private[backend] def assertInserted: F[Unit] = assertInserted(1)
}
