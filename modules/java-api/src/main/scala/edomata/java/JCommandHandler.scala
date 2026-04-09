/*
 * Copyright 2021 Beyond Scale Group
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

package edomata.java

import java.util.function.{Function => JFunction}

/** Java-friendly request context providing command and current state. */
final class JRequestContext[C, S](
    val command: C,
    val commandMessage: JCommandMessage[C],
    val state: S
) {
  def address: String = commandMessage.address
  def messageId: String = commandMessage.id
}

/** Java-friendly command handler wrapping a function from context to result.
  *
  * @tparam C
  *   command type
  * @tparam S
  *   state type
  * @tparam E
  *   event type
  * @tparam R
  *   rejection type
  * @tparam N
  *   notification type
  */
final class JCommandHandler[C, S, E, R, N] private (
    private[java] val handler: JFunction[
      JRequestContext[C, S],
      JAppResult[R, E, N]
    ]
) {
  def apply(ctx: JRequestContext[C, S]): JAppResult[R, E, N] =
    handler.apply(ctx)
}

object JCommandHandler {
  def create[C, S, E, R, N](
      handler: JFunction[JRequestContext[C, S], JAppResult[R, E, N]]
  ): JCommandHandler[C, S, E, R, N] =
    new JCommandHandler(handler)
}
