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

package edomata.skunk

import _root_.skunk.data.Completion
import cats.Functor
import cats.MonadError
import cats.effect.kernel.Clock
import cats.implicits.*
import edomata.backend.*

import java.time.Instant
import java.time.OffsetDateTime
import java.time.ZoneOffset

private def currentTime[F[_]: Functor](using
    clock: Clock[F]
): F[OffsetDateTime] =
  clock.realTime.map(d =>
    Instant.ofEpochMilli(d.toMillis).atOffset(ZoneOffset.UTC)
  )

extension [F[_]](self: F[Completion])(using F: MonadError[F, Throwable]) {
  private[skunk] def assertInserted(size: Int): F[Unit] = self.flatMap {
    case Completion.Insert(i) =>
      if i == size then F.unit
      else
        F.raiseError(
          BackendError.PersistenceError(
            s"expected to insert exactly $size, but inserted $i"
          )
        )
    case other =>
      F.raiseError(
        BackendError.PersistenceError(
          s"expected to receive insert response, but received: $other"
        )
      )
  }
  private[skunk] def assertInserted: F[Unit] = assertInserted(1)
}
