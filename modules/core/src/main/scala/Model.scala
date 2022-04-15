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
