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

import cats.data.NonEmptyChain
import cats.implicits.*

import java.time.Instant

final case class RequestContext[+C, +S](
    command: CommandMessage[C],
    state: S
)

final case class CommandMessage[+C](
    id: String,
    time: Instant,
    address: String,
    payload: C,
    metadata: MessageMetadata
)
object CommandMessage {
  def apply[C](
      id: String,
      time: Instant,
      address: String,
      payload: C
  ): CommandMessage[C] = CommandMessage(
    id = id,
    time = time,
    address = address,
    payload,
    MessageMetadata(id)
  )

  extension [C, M](cmd: CommandMessage[C]) {
    def buildContext[S, R](
        state: S
    ): RequestContext[C, S] =
      RequestContext(
        command = cmd,
        state = state
      )

    def deriveMeta: MessageMetadata = cmd.metadata.copy(causation = cmd.id.some)
  }
}

final case class MessageMetadata(
    correlation: Option[String],
    causation: Option[String]
)
object MessageMetadata {
  def apply(id: String): MessageMetadata = MessageMetadata(id.some, id.some)
  def apply(correlation: String, causation: String): MessageMetadata =
    MessageMetadata(correlation = correlation.some, causation = causation.some)
}
