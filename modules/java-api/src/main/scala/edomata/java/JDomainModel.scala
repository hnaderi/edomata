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

import cats.data.ValidatedNec
import edomata.core.DomainModel
import edomata.core.ModelTC

import java.util.function.BiFunction

abstract class JDomainModel[State, Event, Rejection] {

  /** Initial (empty) state for this aggregate. */
  def initial: State

  /** Transition function: given an event and the current state, produce either
    * the new state or a list of rejections.
    */
  def transition(
      event: Event,
      state: State
  ): JEither[java.util.List[Rejection], State]

  private[java] lazy val toModelTC: ModelTC[State, Event, Rejection] = {
    val self = this
    val dm = new DomainModel[State, Event, Rejection] {
      def initial: State = self.initial
      def transition: Event => State => ValidatedNec[Rejection, State] =
        event =>
          state =>
            self.transition(event, state) match {
              case JEither.Right(newState) =>
                cats.data.Validated.validNec(newState)
              case JEither.Left(reasons) =>
                import scala.jdk.CollectionConverters.*
                val list = reasons.asScala.toList
                list match {
                  case head :: tail =>
                    cats.data.Validated.Invalid(
                      cats.data.NonEmptyChain(head, tail*)
                    )
                  case Nil =>
                    throw new IllegalArgumentException(
                      "Rejection must have at least one reason"
                    )
                }
            }
    }
    dm.given_ModelTC_State_Event_Rejection
  }
}

object JDomainModel {
  def create[State, Event, Rejection](
      initialState: State,
      transitionFn: BiFunction[
        Event,
        State,
        JEither[java.util.List[Rejection], State]
      ]
  ): JDomainModel[State, Event, Rejection] =
    new JDomainModel[State, Event, Rejection] {
      def initial: State = initialState
      def transition(
          event: Event,
          state: State
      ): JEither[java.util.List[Rejection], State] =
        transitionFn.apply(event, state)
    }
}
