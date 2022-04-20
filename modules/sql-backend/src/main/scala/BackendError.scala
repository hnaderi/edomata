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

sealed trait BackendError extends Throwable
object BackendError {
  case object VersionConflict
      extends Throwable(
        "You can't proceed due to version conflict, read and decide again!"
      ),
        BackendError
  case object MaxRetryExceeded
      extends Throwable("Maximum number of retries exceeded!"),
        BackendError
  final case class UnknownError(underlying: Throwable)
      extends Throwable("Unknown error!", underlying),
        BackendError
  final case class PersistenceError(msg: String)
      extends Throwable(msg),
        BackendError
}
