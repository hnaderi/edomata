package edomata.core

import cats.data.EitherNec
import cats.data.NonEmptyChain
import cats.data.ValidatedNec
import cats.implicits.*

import Decision.*

trait Model[S, Event, Rejection] { self: S =>
  def handle[T](
      dec: Decision[Rejection, Event, T]
  ): Decision[Rejection, Event, (S, T)] =
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
  ): Decision[Rejection, Event, S] =
    handle(dec).map(_._1)

  private def applyNec(
      es: NonEmptyChain[Event]
  ): EitherNec[Rejection, S & Model[S, Event, Rejection]] =
    es.foldM(self)((ns, e) => ns.transition(e).toEither)

  type F[T] = Decision[Rejection, Event, T]
  type Transition =
    Event => ValidatedNec[Rejection, S & Model[S, Event, Rejection]]

  def transition: Transition
}

object Model {
  type EventFrom[T] = T match {
    case Model[_, e, _] => e
  }

  type RejectionFrom[T] = T match {
    case Model[_, _, r] => r
  }
}
