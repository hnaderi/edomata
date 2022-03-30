package edomata.core

import cats.Applicative
import cats.Monad
import cats.data.NonEmptyChain
import cats.data.ValidatedNec
import cats.implicits.*

import java.time.Instant

import Domain.*

final class DomainDSL2[C, S, E, R, N, M] {
  type DomainModel = S & Model[S, E, R]
  type Decision[T] = edomata.core.Decision[R, E, T]
  type Logic[F[_], T] = DecisionT[F, R, E, T]
  type LogicOf[F[_]] = [t] =>> DecisionT[F, R, E, t]

  type Context = RequestContext2[C, S, M]
  type RequestOf[F[_]] = [t] =>> RequestMonad[F, Context, N, t]
  type Service[F[_], T] = DecisionT[RequestOf[F], R, E, T]
  type ServiceOf[F[_]] = [t] =>> Service[F, t]

  def router[F[_]: Monad](
      f: C => Service[F, Unit]
  ): Service[F, Unit] =
    command.flatMap(f)

  def eval[F[_]: Monad, T](ft: F[T]): Service[F, T] =
    DecisionT.liftF(RequestMonad.liftF(ft))

  def ask[F[_]: Monad]: Service[F, Context] = DecisionT.liftF(RequestMonad.ask)
  def command[F[_]: Monad]: Service[F, C] = ask.map(_.command)
  def metadata[F[_]: Monad]: Service[F, M] = ask.map(_.metadata)
  def aggregateId[F[_]: Monad]: Service[F, String] = ask.map(_.aggregateId)
  def messageId[F[_]: Monad]: Service[F, String] = ask.map(_.id)
  def state[F[_]: Monad]: Service[F, S] = ask.map(_.state)

  def accept[F[_]: Monad](e: E, es: E*): Service[F, Unit] =
    DecisionT.accept(e, es: _*)

  def acceptReturn[F[_]: Monad, T](t: T)(e: E, es: E*): Service[F, T] =
    DecisionT.acceptReturn(t)(e, es: _*)

  def reject[F[_]: Monad, T](reason: R, more: R*): Service[F, T] =
    DecisionT.lift(Decision.reject(reason, more: _*))

  def validate[F[_]: Monad, T](v: ValidatedNec[R, T]): Service[F, T] =
    DecisionT.validate(v)

  def pure[F[_]: Monad, T](t: T): Service[F, T] =
    DecisionT.pure(t)

  def publish[F[_]: Monad](n: N*): Service[F, Unit] =
    DecisionT.liftF(RequestMonad.publish(n: _*))

  def publishReturn[F[_]: Monad, T](t: T)(n: N*): Service[F, T] =
    DecisionT.liftF(RequestMonad.publishReturn(t)(n: _*))

  def handle[F[_]: Monad, T](
      decision: Decision[T]
  ): Service[F, T] =
    DecisionT.lift(decision)
}
