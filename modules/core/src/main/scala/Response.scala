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

package edomata.core

/** Representation of programs that decide and emit notifications
  *
  * This adds capability of emiting notifications/integration events to
  * [[Decision]] programs
  *
  * @tparam R
  *   rejection type
  * @tparam E
  *   domain event type
  * @tparam N
  *   notification type
  * @tparam A
  *   output type
  */
@deprecated("Use ResponseD instead")
type Response[+R, +E, +N, +A] = ResponseD[R, E, N, A]
@deprecated("Use ResponseD instead")
val Response = ResponseD
