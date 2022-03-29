package edomata.core

import cats.Applicative
import cats.Monad
import cats.data.ValidatedNec
import cats.implicits.*
import edomata.core.*

final class ServiceBuilderDSL[C, S, E, R, N](domain: NewDomain[C, S, E, R, N]) {
  def router[F[_]: Monad](
      f: C => domain.Service[F, Unit]
  ): domain.Service[F, Unit] =
    DecisionT.liftF(RequestMonad.ask).flatMap(f)

  def eval[F[_]: Monad, T](ft: F[T]): domain.Service[F, T] =
    DecisionT.liftF(RequestMonad.liftF(ft))

  def accept[F[_]: Monad](e: E, es: E*): domain.Service[F, Unit] =
    DecisionT.accept(e, es: _*)

  def acceptReturn[F[_]: Monad, T](t: T)(e: E, es: E*): domain.Service[F, T] =
    DecisionT.acceptReturn(t)(e, es: _*)

  def reject[F[_]: Monad](reason: R, more: R*): domain.Service[F, Nothing] =
    DecisionT.reject(reason, more: _*)

  def validate[F[_]: Monad, T](v: ValidatedNec[R, T]): domain.Service[F, T] =
    DecisionT.validate(v)

  def pure[F[_]: Monad, T](t: T): domain.Service[F, T] =
    DecisionT.pure(t)

  def publish[F[_]: Monad](n: N*): domain.Service[F, Unit] =
    DecisionT.liftF(RequestMonad.publish(n: _*))

  def publishReturn[F[_]: Monad, T](t: T)(n: N*): domain.Service[F, T] =
    DecisionT.liftF(RequestMonad.publishReturn(t)(n: _*))

  def handle[F[_]: Monad, T](
      decision: domain.Decision[T]
  ): domain.Service[F, T] =
    DecisionT.lift(decision)
}
