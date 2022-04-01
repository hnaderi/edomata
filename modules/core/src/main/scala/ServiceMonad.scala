package edomata.core

import cats.Applicative
import cats.FlatMap
import cats.Functor
import cats.implicits.*
import cats.data.NonEmptyChain
import cats.Monad
import cats.data.Kleisli
import cats.kernel.Eq
import cats.Contravariant

final case class ServiceMonad[F[_], Env, R, E, N, A](
    run: Env => F[ResponseMonad[R, E, N, A]]
) {
  type G[T] = ServiceMonad[F, Env, R, E, N, T]

  def map[B](f: A => B)(using Functor[F]): G[B] =
    transform(_.map(f))

  def contramap[Env2](f: Env2 => Env): ServiceMonad[F, Env2, R, E, N, A] =
    ServiceMonad(
      run.compose(f)
    )

  def flatMap[B](f: A => G[B])(using Monad[F]): G[B] =
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
  ): G[B] =
    ServiceMonad(run.andThen(_.map(f)))

  def as[B](b: B)(using Functor[F]): G[B] = map(_ => b)

  /** Clears all notifications so far */
  def reset(using Functor[F]): G[A] =
    transform(_.reset)

  /** Adds notification without considering decision state */
  def publish(ns: N*)(using Functor[F]): G[A] =
    transform(_.publish(ns: _*))

  def publishOnRejectionWith(
      f: NonEmptyChain[R] => Seq[N]
  )(using Functor[F]): G[A] = transform(_.publishOnRejectionWith(f))

  def publishOnRejection(ns: N*)(using Functor[F]): G[A] =
    transform(_.publishOnRejection(ns: _*))
}

object ServiceMonad {
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
            _ => ???,
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
