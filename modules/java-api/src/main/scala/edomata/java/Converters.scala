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

import cats.data.Chain
import cats.data.NonEmptyChain
import cats.implicits.*
import edomata.core.Decision

import scala.jdk.CollectionConverters.*

private[java] object Converters {
  def toJavaList[A](chain: Chain[A]): java.util.List[A] =
    chain.toList.asJava

  def toJavaList[A](nec: NonEmptyChain[A]): java.util.List[A] =
    nec.toList.asJava

  def toChain[A](list: java.util.List[A]): Chain[A] =
    Chain.fromSeq(list.asScala.toSeq)

  def toNonEmptyChain[A](list: java.util.List[A]): Option[NonEmptyChain[A]] =
    NonEmptyChain.fromSeq(list.asScala.toSeq)

  def decisionToJava[R, E, A](
      d: Decision[R, E, A]
  ): JDecision[R, E, A] = d match {
    case Decision.Accepted(events, result) =>
      JDecision.Accepted(toJavaList(events), result)
    case Decision.Rejected(reasons) =>
      JDecision.Rejected(toJavaList(reasons))
    case Decision.InDecisive(result) =>
      JDecision.Indecisive(result)
  }

  def decisionToScala[R, E, A](
      d: JDecision[R, E, A]
  ): Decision[R, E, A] = d match {
    case JDecision.Accepted(events, result) =>
      toNonEmptyChain(events) match {
        case Some(nec) => Decision.Accepted(nec, result)
        case None      => Decision.InDecisive(result)
      }
    case JDecision.Rejected(reasons) =>
      toNonEmptyChain(reasons) match {
        case Some(nec) => Decision.Rejected(nec)
        case None      =>
          throw new IllegalArgumentException(
            "Rejected must have at least one reason"
          )
      }
    case JDecision.Indecisive(result) =>
      Decision.InDecisive(result)
  }
}
