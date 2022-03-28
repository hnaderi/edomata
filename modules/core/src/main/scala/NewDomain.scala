package edomata.core

import cats.Applicative
import cats.Monad
import cats.data.ValidatedNec
import cats.implicits.*
import edomata.core.*

sealed trait NewDomain[Command, State, Event, Rejection, Notification] {
  def withCommand[T]: NewDomain[T, State, Event, Rejection, Notification] =
    new NewDomain {}
  def withState[T]: NewDomain[Command, T, Event, Rejection, Notification] =
    new NewDomain {}
  def withEvent[T]: NewDomain[Command, State, T, Rejection, Notification] =
    new NewDomain {}
  def withRejection[T]: NewDomain[Command, State, Event, T, Notification] =
    new NewDomain {}
  def withNotification[T]: NewDomain[Command, State, Event, Rejection, T] =
    new NewDomain {}

  type DomainModel = State & Model[State, Rejection, Event]
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
