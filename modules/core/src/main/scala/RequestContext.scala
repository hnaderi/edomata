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

/** Representation of a standard input for an [[Edomaton]]
  *
  * @tparam C
  *   Command type
  * @tparam S
  *   State model type
  */
final case class RequestContext[+C, +S](
    command: CommandMessage[C],
    state: S
)

/** Representation of a standard command message
  *
  * @tparam C
  *   Command payload which is your command model
  */
final case class CommandMessage[+C](
    id: String,
    time: Instant,
    address: String,
    payload: C,
    metadata: MessageMetadata
)

object CommandMessage {

  /** Constructs a command message a the root of a chain of messages */
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

    /** Builds a request context from this [[CommandMessage]] */
    def buildContext[S, R](
        state: S
    ): RequestContext[C, S] =
      RequestContext(
        command = cmd,
        state = state
      )

    /** Derives a new metadata for the next message in chain that is linked to
      * this one
      */
    def deriveMeta: MessageMetadata = cmd.metadata.copy(causation = cmd.id.some)
  }
}

/** Representation of a standard message metadata */
final case class MessageMetadata(
    correlation: Option[String],
    causation: Option[String]
)
object MessageMetadata {

  /** Consturcts a root chain metadata */
  def apply(id: String): MessageMetadata = MessageMetadata(id.some, id.some)

  /** Consturcts a metadata */
  def apply(correlation: String, causation: String): MessageMetadata =
    MessageMetadata(correlation = correlation.some, causation = causation.some)
}
