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

import cats.effect.Temporal
import cats.effect.implicits.*
import cats.effect.std.Random
import cats.implicits.*
import scala.concurrent.duration.*

private[backend] def retry[F[_]: Temporal: Random, T](
    max: Int,
    wait: FiniteDuration
)(
    f: F[T]
): F[T] =
  f.recoverWith {
    case BackendError.VersionConflict if max > 1 =>
      Random[F]
        .nextIntBounded(500)
        .map(_.millis)
        .flatMap(jitter => retry(max - 1, wait * 2)(f).delayBy(wait + jitter))
  }.adaptErr { case BackendError.VersionConflict =>
    BackendError.MaxRetryExceeded
  }
