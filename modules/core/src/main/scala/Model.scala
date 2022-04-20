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

import cats.data.EitherNec
import cats.data.NonEmptyChain
import cats.data.ValidatedNec
import cats.implicits.*

import scala.annotation.implicitAmbiguous
import scala.annotation.implicitNotFound

import Decision.*

@implicitNotFound("Cannot find domain model definition")
@implicitAmbiguous("Domain model definition must be unique!")
sealed trait ModelTC[State, Event, Rejection] {
  def initial: State
  def transition: Event => State => ValidatedNec[Rejection, State]
}

abstract class DomainModel[State, Event, Rejection] { self =>
  def initial: State
  def transition: Event => State => ValidatedNec[Rejection, State]

  given ModelTC[State, Event, Rejection] = new {
    def initial = self.initial
    def transition = self.transition
  }
}

extension [State, Event, Rejection](
    self: State
)(using m: ModelTC[State, Event, Rejection]) {
  final def handle[T](
      dec: Decision[Rejection, Event, T]
  ): Decision[Rejection, Event, (State, T)] =
    dec match {
      case d @ Decision.Accepted(es, v) =>
        applyNec(es)(self).fold(
          Decision.Rejected(_),
          s => Decision.Accepted(es, (s, v))
        )
      case d @ Decision.InDecisive(v) => Decision.pure((self, v))
      case d @ Decision.Rejected(_)   => d.copy()
    }

  final def perform(
      dec: Decision[Rejection, Event, Unit]
  ): Decision[Rejection, Event, State] =
    handle(dec).map(_._1)

  private def applyNec(
      es: NonEmptyChain[Event]
  ): State => EitherNec[Rejection, State] = self =>
    es.foldM(self)((ns, e) => m.transition(e)(ns).toEither)
}
