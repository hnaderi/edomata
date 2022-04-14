package edomata.core

import cats.data.EitherNec
import cats.data.NonEmptyChain
import cats.data.ValidatedNec
import cats.implicits.*

import Decision.*

trait Model[State, Event, Rejection] { self: State =>
  def handle[T](
      dec: Decision[Rejection, Event, T]
  ): Decision[Rejection, Event, (Model.Of[State, Event, Rejection], T)] =
    dec match {
      case d @ Decision.Accepted(es, v) =>
        applyNec(es).fold(
          Decision.Rejected(_),
          s => Decision.Accepted(es, (s, v))
        )
      case d @ Decision.InDecisive(v) => Decision.pure((self, v))
      case d @ Decision.Rejected(_)   => d.copy()
    }

  def perform(
      dec: Decision[Rejection, Event, Unit]
  ): Decision[Rejection, Event, Model.Of[State, Event, Rejection]] =
    handle(dec).map(_._1)

  private def applyNec(
      es: NonEmptyChain[Event]
  ): EitherNec[Rejection, Model.Of[State, Event, Rejection]] =
    es.foldM(self)((ns, e) => ns.transition(e).toEither)

  type F[T] = Decision[Rejection, Event, T]
  type Transition =
    Event => ValidatedNec[Rejection, Model.Of[State, Event, Rejection]]

  def transition: Transition
}

object Model {
  type EventFrom[T] = T match {
    case Model[_, e, _] => e
  }

  type RejectionFrom[T] = T match {
    case Model[_, _, r] => r
  }

  type Of[S, E, R] = S & Model[S, E, R]
}
