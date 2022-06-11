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

import cats.Applicative
import cats.data.EitherNec
import cats.data.NonEmptyChain
import cats.data.ValidatedNec
import cats.implicits.*

import scala.annotation.implicitAmbiguous
import scala.annotation.implicitNotFound

import Decision.*

/** A type class that captures domain model
  *
  * note that due to uniqueness requirements for this typeclass, it is sealed
  * and the only way to create an instance is through implementing DomainModel.
  *
  * so don't create several instances as it is a bad idea and may change your
  * domain model behavior in different contexts!
  */
@implicitNotFound(
  "Cannot find domain model definition for State: ${State}, Event: ${Event}, Rejection: ${Rejection}"
)
@implicitAmbiguous("Domain model definition must be unique!")
sealed trait ModelTC[State, Event, Rejection] {
  def initial: State
  def transition: Event => State => ValidatedNec[Rejection, State]

  /** like perform, but also returns initial output */
  final def handle[T](
      self: State,
      dec: Decision[Rejection, Event, T]
  ): Decision[Rejection, Event, (State, T)] =
    dec match {
      case d @ Decision.Accepted(es, v) =>
        applyNec(self, es).fold(
          Decision.Rejected(_),
          s => Decision.Accepted(es, (s, v))
        )
      case d @ Decision.InDecisive(v) => Decision.pure((self, v))
      case d @ Decision.Rejected(_)   => d.copy()
    }

  /** Returns a decision that has applied this decision and folded state, so the
    * output is new state
    */
  final def perform(
      self: State,
      dec: Decision[Rejection, Event, Unit]
  ): Decision[Rejection, Event, State] =
    handle(self, dec).map(_._1)

  private def applyNec(
      self: State,
      es: NonEmptyChain[Event]
  ): EitherNec[Rejection, State] =
    es.foldM(self)((ns, e) => transition(e)(ns).toEither)
}

/** A purely functional, event driven domain model
  *
  * @tparam State
  *   state model of your program, a.k.a aggregate root
  * @tparam Event
  *   domain events
  * @tparam Rejection
  *   domain error type
  */
trait DomainModel[State, Event, Rejection] { self =>

  /** Initial or empty value for this domain model
    *
    * for any aggregate, it is either created and have a history, or is in
    * initial state
    */
  def initial: State

  /** A function that defines how this model transitions in response to events;
    * it is like an event handler, but is pure.
    *
    * An event that can't be applied results in a rejection or conflict, based
    * on whether it is read from journal or applied for decision
    */
  def transition: Event => State => ValidatedNec[Rejection, State]

  given ModelTC[State, Event, Rejection] = new {
    def initial = self.initial
    def transition = self.transition
  }

  trait Service[C, N] {
    final type App[F[_], T] =
      Edomaton[F, RequestContext[C, State], Rejection, Event, N, T]

    final type PureApp[T] = App[cats.Id, T]

    final type Handler[F[_]] = DomainService[F, CommandMessage[C], Rejection]

    final protected val App: DomainDSL[C, State, Event, Rejection, N] =
      DomainDSL()

    final val domain: Domain[C, State, Event, Rejection, N] = Domain()

    extension [T](dec: Decision[Rejection, Event, T]) {
      def toApp[F[_]: Applicative]: App[F, T] = App.decide(dec)
    }
  }
}

private[edomata] transparent trait ModelSyntax {
  extension [State, Event, Rejection](
      self: State
  )(using m: ModelTC[State, Event, Rejection]) {

    /** like perform, but also returns initial output */
    def handle[T](
        dec: Decision[Rejection, Event, T]
    ): Decision[Rejection, Event, (State, T)] =
      m.handle(self, dec)

    /** Returns a decision that has applied this decision and folded state, so
      * the output is new state
      */
    def perform[T](
        dec: Decision[Rejection, Event, T]
    ): Decision[Rejection, Event, State] =
      m.perform(self, dec.void)

    /** Helps with deciding based on state and then applying the decision */
    def decide[T](
        f: State => Decision[Rejection, Event, T]
    ): Decision[Rejection, Event, State] =
      m.perform(self, f(self).void)

    /** Helps with deciding based on state and then applying the decision */
    def decideReturn[T](
        f: State => Decision[Rejection, Event, T]
    ): Decision[Rejection, Event, (State, T)] =
      m.handle(self, f(self))

    /** Applies events to state, returns new state */
    def accept(ev: Event, evs: Event*): Decision[Rejection, Event, State] =
      m.perform(self, Decision.accept(ev, evs: _*))
  }
}
