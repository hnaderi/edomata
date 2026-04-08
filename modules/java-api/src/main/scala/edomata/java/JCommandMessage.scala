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

import edomata.core.CommandMessage

import java.time.Instant

final class JCommandMessage[C] private (
    private[java] val underlying: CommandMessage[C]
) {
  def id: String = underlying.id
  def time: Instant = underlying.time
  def address: String = underlying.address
  def payload: C = underlying.payload

  override def toString: String =
    s"JCommandMessage(id=$id, time=$time, address=$address, payload=$payload)"

  override def equals(obj: Any): Boolean = obj match {
    case other: JCommandMessage[?] => underlying == other.underlying
    case _                         => false
  }

  override def hashCode(): Int = underlying.hashCode()
}

object JCommandMessage {
  def of[C](
      id: String,
      time: Instant,
      address: String,
      payload: C
  ): JCommandMessage[C] =
    new JCommandMessage(CommandMessage(id, time, address, payload))

  private[java] def fromScala[C](cmd: CommandMessage[C]): JCommandMessage[C] =
    new JCommandMessage(cmd)
}
