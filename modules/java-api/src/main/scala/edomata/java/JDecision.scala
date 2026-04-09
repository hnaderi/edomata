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

sealed abstract class JDecision[+R, +E, +A] {
  def isAccepted: Boolean
  def isRejected: Boolean
  def isIndecisive: Boolean

  def map[A2 >: A, B](f: JFunction[A2, B]): JDecision[R, E, B] =
    this match {
      case JDecision.Accepted(events, result) =>
        JDecision.Accepted(events, f.apply(result))
      case JDecision.Rejected(reasons) =>
        JDecision.Rejected(reasons)
      case JDecision.Indecisive(result) =>
        JDecision.Indecisive(f.apply(result))
    }

  def flatMap[R2 >: R, E2 >: E, A2 >: A, B](
      f: JFunction[A2, JDecision[R2, E2, B]]
  ): JDecision[R2, E2, B] =
    this match {
      case JDecision.Accepted(events, result) =>
        f.apply(result) match {
          case JDecision.Accepted(events2, result2) =>
            val merged = new java.util.ArrayList[E2](events.size + events2.size)
            events.forEach(e => { merged.add(e); () })
            events2.forEach(e => { merged.add(e); () })
            JDecision.Accepted(
              java.util.Collections.unmodifiableList(merged),
              result2
            )
          case JDecision.Rejected(reasons) =>
            JDecision.Rejected(reasons)
          case JDecision.Indecisive(result2) =>
            JDecision.Accepted(events, result2)
        }
      case JDecision.Rejected(reasons) =>
        JDecision.Rejected(reasons)
      case JDecision.Indecisive(result) =>
        f.apply(result)
    }

  def toEither: JEither[
    java.util.List[? <: R @scala.annotation.unchecked.uncheckedVariance],
    A
  ] =
    this match {
      case JDecision.Accepted(_, result) => JEither.Right(result)
      case JDecision.Rejected(reasons)   => JEither.Left(reasons)
      case JDecision.Indecisive(result)  => JEither.Right(result)
    }
}

object JDecision {
  final case class Accepted[+E, +A](
      events: java.util.List[
        ? <: E @scala.annotation.unchecked.uncheckedVariance
      ],
      result: A
  ) extends JDecision[Nothing, E, A] {
    def isAccepted: Boolean = true
    def isRejected: Boolean = false
    def isIndecisive: Boolean = false
  }

  final case class Rejected[+R](
      reasons: java.util.List[
        ? <: R @scala.annotation.unchecked.uncheckedVariance
      ]
  ) extends JDecision[R, Nothing, Nothing] {
    def isAccepted: Boolean = false
    def isRejected: Boolean = true
    def isIndecisive: Boolean = false
  }

  final case class Indecisive[+A](result: A)
      extends JDecision[Nothing, Nothing, A] {
    def isAccepted: Boolean = false
    def isRejected: Boolean = false
    def isIndecisive: Boolean = true
  }

  @scala.annotation.varargs
  def accept[R, E](event: E, events: E*): JDecision[R, E, Unit] = {
    val list = new java.util.ArrayList[E](1 + events.size)
    list.add(event)
    events.foreach(list.add)
    Accepted(java.util.Collections.unmodifiableList(list), ())
  }

  @scala.annotation.varargs
  def acceptReturn[R, E, A](
      result: A,
      event: E,
      events: E*
  ): JDecision[R, E, A] = {
    val list = new java.util.ArrayList[E](1 + events.size)
    list.add(event)
    events.foreach(list.add)
    Accepted(java.util.Collections.unmodifiableList(list), result)
  }

  @scala.annotation.varargs
  def reject[R, E](reason: R, reasons: R*): JDecision[R, E, Unit] = {
    val list = new java.util.ArrayList[R](1 + reasons.size)
    list.add(reason)
    reasons.foreach(list.add)
    Rejected(java.util.Collections.unmodifiableList(list))
  }

  def pure[R, E, A](value: A): JDecision[R, E, A] = Indecisive(value)

  def unit[R, E]: JDecision[R, E, Unit] = Indecisive(())
}
