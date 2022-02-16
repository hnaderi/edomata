package edomata.eventsourcing

import cats.data.ValidatedNec

type Transition[S] = Either[String, S]
object Transition {
  def next[S](s: S): Transition[S] = Right(s)
  def impossible[S](debug: String): Transition[S] = Left(debug)
}

type Fold[S, E, R] = E => S => ValidatedNec[R, S]
