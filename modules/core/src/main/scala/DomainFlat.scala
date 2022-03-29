package edomata.core

import cats.data.ValidatedNec
import cats.implicits.*

import Model.{EventFrom, RejectionFrom}

sealed trait DomainFlat[Command, State, Event, Rejection, Notification] {
  def withCommand[T]: DomainFlat[T, State, Event, Rejection, Notification] =
    new DomainFlat {}
  def withNotification[T]: DomainFlat[Command, State, Event, Rejection, T] =
    new DomainFlat {}

  def withModel[T <: Model[?, ?, ?]]
      : DomainFlat[Command, T, EventFrom[T], RejectionFrom[T], Notification] =
    new DomainFlat {}

  def withCMD[M[_] <: CommandMetadata[_], T]
      : DomainFlat[T, State, Event, Rejection, Notification] =
    new DomainFlat {}

  type DomainModel = State & Model[State, Event, Rejection]
  type Decision[T] = edomata.core.Decision[Rejection, Event, T]
  type Logic[F[_], T] = DecisionT[F, Rejection, Event, T]
  type LogicOf[F[_]] = [t] =>> DecisionT[F, Rejection, Event, t]

  type RequestOf[F[_]] = [t] =>> RequestMonad[F, Command, Notification, t]
  type Service[F[_], T] = DecisionT[RequestOf[F], Rejection, Event, T]
}

object DomainFlat {
  type EmptyDomain = DomainFlat[Nothing, Nothing, Nothing, Nothing, Nothing]

  def builder: EmptyDomain = new DomainFlat {}
}
