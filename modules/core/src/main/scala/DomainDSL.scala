package edomata.core

import cats.Applicative
import cats.Monad
import cats.data.NonEmptyChain
import cats.data.ValidatedNec
import cats.implicits.*

import java.time.Instant

final class DomainDSL[F[_]: Monad, C, S, E, R, N, M] {
  type DomainModel = S & Model[S, E, R]
  type Decision[T] = edomata.core.Decision[R, E, T]
  type Logic[F[_], T] = DecisionT[F, R, E, T]
  type LogicOf[F[_]] = [t] =>> DecisionT[F, R, E, t]

  type Context = RequestContext2[C, S, M]
  type RequestOf[F[_]] = [t] =>> RequestMonad[F, Context, N, t]
  type Service[F[_], T] = DecisionT[RequestOf[F], R, E, T]
  type ServiceOf[F[_]] = [t] =>> Service[F, t]

  def router(
      f: C => Service[F, Unit]
  ): Service[F, Unit] =
    command.flatMap(f)

  def eval[T](ft: F[T]): Service[F, T] =
    DecisionT.liftF(RequestMonad.liftF(ft))

  def ask: Service[F, Context] =
    DecisionT.liftF(RequestMonad.ask)
  def command: Service[F, C] = ask.map(_.command)
  def metadata: Service[F, M] = ask.map(_.metadata)
  def aggregateId: Service[F, String] = ask.map(_.aggregateId)
  def messageId: Service[F, String] = ask.map(_.id)
  def state: Service[F, S] = ask.map(_.state)

  def accept(e: E, es: E*): Service[F, Unit] =
    DecisionT.accept(e, es: _*)

  def acceptReturn[T](t: T)(e: E, es: E*): Service[F, T] =
    DecisionT.acceptReturn(t)(e, es: _*)

  def reject[T](reason: R, more: R*): Service[F, T] =
    DecisionT.lift(Decision.reject(reason, more: _*))

  def validate[T](v: ValidatedNec[R, T]): Service[F, T] =
    DecisionT.validate(v)

  def pure[T](t: T): Service[F, T] =
    DecisionT.pure(t)

  def publish(n: N*): Service[F, Unit] =
    DecisionT.liftF(RequestMonad.publish(n: _*))

  def publishReturn[T](t: T)(n: N*): Service[F, T] =
    DecisionT.liftF(RequestMonad.publishReturn(t)(n: _*))

  def handle[T](
      decision: Decision[T]
  ): Service[F, T] =
    DecisionT.lift(decision)
}

object DomainDSL {
  import Domain.*
  def build[F[_]: Monad, D]: DomainDSL[F, CommandFor[D], StateFor[D], EventFor[
    D
  ], RejectionFor[D], NotificationFor[D], MetadataFor[D]] = new DomainDSL
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