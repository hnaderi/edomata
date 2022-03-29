package edomata.core

import cats.Applicative
import cats.Monad
import cats.data.NonEmptyChain
import cats.data.ValidatedNec
import cats.implicits.*

import java.time.Instant

import Domain.*

final case class DomainDSL[C, S, E, R, N, M[C] <: CommandMetadata[C]](
    domain: HasModel[S, E, R] & HasCommand[C] & HasMetadata[M] &
      HasNotification[N]
) {
  type DomainModel = S & Model[S, E, R]
  type Decision[T] = edomata.core.Decision[R, E, T]
  type Logic[F[_], T] = DecisionT[F, R, E, T]
  type LogicOf[F[_]] = [t] =>> DecisionT[F, R, E, t]

  type RequestOf[F[_]] = [t] =>> RequestMonad[F, M[C], N, t]
  type Service[F[_], T] = DecisionT[RequestOf[F], R, E, T]

  def router[F[_]: Monad](
      f: C => Service[F, Unit]
  ): Service[F, Unit] =
    ask.flatMap(cm => f(cm.payload))

  def eval[F[_]: Monad, T](ft: F[T]): Service[F, T] =
    DecisionT.liftF(RequestMonad.liftF(ft))

  def ask[F[_]: Monad]: Service[F, M[C]] = DecisionT.liftF(RequestMonad.ask)
  def aggregateId[F[_]: Monad]: Service[F, String] = ask.map(_.address)
  def messageId[F[_]: Monad]: Service[F, String] = ask.map(_.id)
  def messageTime[F[_]: Monad]: Service[F, Instant] = ask.map(_.time)
  def command[F[_]: Monad]: Service[F, C] = ask.map(_.payload)

  def accept[F[_]: Monad](e: E, es: E*): Service[F, Unit] =
    DecisionT.accept(e, es: _*)

  def acceptReturn[F[_]: Monad, T](t: T)(e: E, es: E*): Service[F, T] =
    DecisionT.acceptReturn(t)(e, es: _*)

  def reject[F[_]: Monad](reason: R, more: R*): Service[F, Nothing] =
    DecisionT.reject(reason, more: _*)

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

type ServiceF[F[_], C, R, E, N, T] =
  DecisionT[[t] =>> RequestMonad[F, C, N, t], R, E, T]

extension [F[_]: Monad, C, R, E, N, T](service: ServiceF[F, C, R, E, N, T]) {
  def exec(c: C): F[Response[N, Decision[R, E, T]]] = service.run.run(c)
  def publishOnRejectionNoResetWith(
      f: NonEmptyChain[R] => Seq[N]
  ): ServiceF[F, C, R, E, N, T] = service.onError { case e =>
    DecisionT.liftF(RequestMonad.publish(f(e): _*))
  }
  def publishOnRejectionNoReset(ns: N*): ServiceF[F, C, R, E, N, T] =
    publishOnRejectionNoResetWith(_ => ns)

  def publishOnRejectionWith(
      f: NonEmptyChain[R] => Seq[N]
  ): ServiceF[F, C, R, E, N, T] = DecisionT {
    RequestMonad { env =>
      service.run.run(env).map {
        case res @ Response(Decision.Rejected(e), _) =>
          res.copy(notifications = f(e))
        case other => other
      }
    }
  }
  def publishOnRejection(ns: N*): ServiceF[F, C, R, E, N, T] =
    publishOnRejectionWith(_ => ns)

  def resetOnRejection: ServiceF[F, C, R, E, N, T] =
    publishOnRejectionWith(_ => Nil)
}
