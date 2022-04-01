package edomata.core

import cats.Applicative
import cats.Contravariant
import cats.FlatMap
import cats.Functor
import cats.Monad
import cats.arrow.FunctionK
import cats.data.NonEmptyChain
import cats.implicits.*
import cats.kernel.Eq

import ServiceMonad.*

final case class ServiceMonad[F[_], -Env, R, E, N, A](
    run: Env => F[ResponseMonad[R, E, N, A]]
) {
  def map[B](f: A => B)(using Functor[F]): ServiceMonad[F, Env, R, E, N, B] =
    transform(_.map(f))

  def contramap[Env2](f: Env2 => Env): ServiceMonad[F, Env2, R, E, N, A] =
    ServiceMonad(
      run.compose(f)
    )

  def flatMap[Env2 <: Env, B](
      f: A => ServiceMonad[F, Env2, R, E, N, B]
  )(using Monad[F]): ServiceMonad[F, Env2, R, E, N, B] =
    ServiceMonad(env =>
      run(env).flatMap { r =>
        r.result.fold(
          err => r.copy(result = Decision.Rejected(err)).pure[F],
          f(_)
            .run(env)
            .map(o =>
              o.result.fold(
                _ => o,
                _ => r >> o
              )
            )
        )
      }
    )

  def transform[B](f: ResponseMonad[R, E, N, A] => ResponseMonad[R, E, N, B])(
      using Functor[F]
  ): ServiceMonad[F, Env, R, E, N, B] =
    ServiceMonad(run.andThen(_.map(f)))

  def mapK[G[_]](fk: FunctionK[F, G]): ServiceMonad[G, Env, R, E, N, A] =
    ServiceMonad(run.andThen(fk.apply))

  def andThen[B](f: A => F[B])(using
      Monad[F]
  ): ServiceMonad[F, Env, R, E, N, B] =
    flatMap(a => liftF(f(a).map(ResponseMonad.pure)))

  def as[B](b: B)(using Functor[F]): ServiceMonad[F, Env, R, E, N, B] =
    map(_ => b)

  /** Clears all notifications so far */
  def reset(using Functor[F]): ServiceMonad[F, Env, R, E, N, A] =
    transform(_.reset)

  /** Adds notification without considering decision state */
  def publish(ns: N*)(using Functor[F]): ServiceMonad[F, Env, R, E, N, A] =
    transform(_.publish(ns: _*))

  def publishOnRejectionWith(
      f: NonEmptyChain[R] => Seq[N]
  )(using Functor[F]): ServiceMonad[F, Env, R, E, N, A] = transform(
    _.publishOnRejectionWith(f)
  )

  def publishOnRejection(ns: N*)(using
      Functor[F]
  ): ServiceMonad[F, Env, R, E, N, A] =
    transform(_.publishOnRejection(ns: _*))
}

object ServiceMonad
    extends ServiceMonadInstances
    with ServiceMonadConstructors
    with DomainServiceConstructors

sealed trait ServiceMonadInstances {
  given [F[_]: Monad, Env, R, E, N]
      : Monad[[t] =>> ServiceMonad[F, Env, R, E, N, t]] =
    new Monad {
      type G[T] = ServiceMonad[F, Env, R, E, N, T]
      override def pure[A](x: A): G[A] =
        ServiceMonad(_ => ResponseMonad.pure(x).pure[F])

      override def map[A, B](fa: G[A])(f: A => B): G[B] = fa.map(f)

      override def flatMap[A, B](fa: G[A])(f: A => G[B]): G[B] = fa.flatMap(f)

      override def tailRecM[A, B](a: A)(
          f: A => G[Either[A, B]]
      ): G[B] = ServiceMonad(env =>
        Monad[F].tailRecM(ResponseMonad.pure[R, E, N, A](a))(rma =>
          rma.result.fold(
            _ => ???, //This cannot happen
            a =>
              f(a)
                .run(env)
                .map(rma >> _)
                .map(o =>
                  o.result
                    .fold(
                      rej => o.copy(result = Decision.Rejected(rej)).asRight,
                      {
                        case Left(a)  => o.as(a).asLeft
                        case Right(b) => o.as(b).asRight
                      }
                    )
                )
          )
        )
      )
    }

  given [F[_], Env, R, E, N, T](using
      Eq[Env => F[ResponseMonad[R, E, N, T]]]
  ): Eq[ServiceMonad[F, Env, R, E, N, T]] =
    Eq.by(_.run)

  given [F[_], R, E, N, T]
      : Contravariant[[env] =>> ServiceMonad[F, env, R, E, N, T]] =
    new Contravariant {
      override def contramap[A, B](fa: ServiceMonad[F, A, R, E, N, T])(
          f: B => A
      ): ServiceMonad[F, B, R, E, N, T] = fa.contramap(f)
    }

}

sealed trait ServiceMonadConstructors {
  def pure[F[_]: Monad, Env, R, E, N, T](
      t: T
  ): ServiceMonad[F, Env, R, E, N, T] =
    ServiceMonad(_ => ResponseMonad.pure(t).pure)

  def unit[F[_]: Monad, Env, R, E, N, T]: ServiceMonad[F, Env, R, E, N, Unit] =
    pure(())

  def liftF[F[_], Env, R, E, N, T](
      f: F[ResponseMonad[R, E, N, T]]
  ): ServiceMonad[F, Env, R, E, N, T] = ServiceMonad(_ => f)

  def lift[F[_]: Applicative, Env, R, E, N, T](
      f: ResponseMonad[R, E, N, T]
  ): ServiceMonad[F, Env, R, E, N, T] = liftF(f.pure)

  def eval[F[_]: Applicative, Env, R, E, N, T](
      f: F[T]
  ): ServiceMonad[F, Env, R, E, N, T] = liftF(f.map(ResponseMonad.pure))

  def run[F[_]: Applicative, Env, R, E, N, T](
      f: Env => F[T]
  ): ServiceMonad[F, Env, R, E, N, T] = ServiceMonad(
    f.andThen(_.map(ResponseMonad.pure))
  )

  def map[F[_]: Applicative, Env, R, E, N, T](
      f: Env => T
  ): ServiceMonad[F, Env, R, E, N, T] =
    run(f.andThen(_.pure))

  def read[F[_]: Applicative, Env, R, E, N, T]
      : ServiceMonad[F, Env, R, E, N, Env] = run(_.pure[F])

  def publish[F[_]: Applicative, Env, R, E, N](
      ns: N*
  ): ServiceMonad[F, Env, R, E, N, Unit] = lift(ResponseMonad.publish(ns: _*))

  def reject[F[_]: Applicative, Env, R, E, N](
      r: R,
      rs: R*
  ): ServiceMonad[F, Env, R, E, N, Unit] = lift(ResponseMonad.reject(r, rs: _*))

  def perform[F[_]: Applicative, Env, R, E, N, T](
      d: Decision[R, E, T]
  ): ServiceMonad[F, Env, R, E, N, T] = lift(ResponseMonad.lift(d))

}
