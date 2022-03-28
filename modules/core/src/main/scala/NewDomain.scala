package edomata.core

import cats.data.ValidatedNec
import cats.implicits.*
import edomata.core.*

sealed trait NewDomain[Command, State, Event, Rejection, Notification] {
  def withCommand[T]: NewDomain[T, State, Event, Rejection, Notification] =
    new NewDomain {}
  def withNotification[T]: NewDomain[Command, State, Event, Rejection, T] =
    new NewDomain {}
  def withModel[T <: Model[?, ?, ?]]
      : NewDomain[Command, T, EventFrom[T], RejectionFrom[T], Notification] =
    new NewDomain {}

  type EventFrom[T] = T match {
    case Model[_, e, _] => e
  }

  type RejectionFrom[T] = T match {
    case Model[_, _, r] => r
  }

  type DomainModel = State & Model[State, Event, Rejection]
  type Decision[T] = edomata.core.Decision[Rejection, Event, T]
  type Logic[F[_], T] = DecisionT[F, Rejection, Event, T]
  type LogicOf[F[_]] = [t] =>> DecisionT[F, Rejection, Event, t]

  type RequestOf[F[_]] = [t] =>> RequestMonad[F, Command, Notification, t]
  type Service[F[_], T] = DecisionT[RequestOf[F], Rejection, Event, T]
}

object NewDomain {
  type EmptyDomain = NewDomain[Nothing, Nothing, Nothing, Nothing, Nothing]

  def builder: EmptyDomain = new NewDomain {}
}
