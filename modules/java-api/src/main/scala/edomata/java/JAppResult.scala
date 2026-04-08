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

final class JAppResult[R, E, N] private (
    val decision: JDecision[R, E, Unit],
    val notifications: java.util.List[N]
) {
  override def toString: String =
    s"JAppResult(decision=$decision, notifications=$notifications)"
}

object JAppResult {
  def decide[R, E, N](
      decision: JDecision[R, E, Unit]
  ): JAppResult[R, E, N] =
    new JAppResult(decision, java.util.Collections.emptyList())

  def decideAndPublish[R, E, N](
      decision: JDecision[R, E, Unit],
      notifications: java.util.List[N]
  ): JAppResult[R, E, N] =
    new JAppResult(decision, notifications)

  @scala.annotation.varargs
  def accept[R, E, N](event: E, events: E*): JAppResult[R, E, N] =
    new JAppResult(
      JDecision.accept[R, E](event, events*),
      java.util.Collections.emptyList()
    )

  @scala.annotation.varargs
  def reject[R, E, N](
      reason: R,
      reasons: R*
  ): JAppResult[R, E, N] =
    new JAppResult(
      JDecision.reject[R, E](reason, reasons*),
      java.util.Collections.emptyList()
    )

  def publish[R, E, N](notifications: java.util.List[N]): JAppResult[R, E, N] =
    new JAppResult(JDecision.unit[R, E], notifications)
}
