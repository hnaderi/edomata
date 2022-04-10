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

import Edomaton.*

final case class Edomaton[F[_], -Env, R, E, N, A](
    run: Env => F[Response[R, E, N, A]]
) {
  def map[B](f: A => B)(using Functor[F]): Edomaton[F, Env, R, E, N, B] =
    transform(_.map(f))

  def contramap[Env2](f: Env2 => Env): Edomaton[F, Env2, R, E, N, A] =
    Edomaton(
      run.compose(f)
    )

  def flatMap[Env2 <: Env, B](
      f: A => Edomaton[F, Env2, R, E, N, B]
  )(using Monad[F]): Edomaton[F, Env2, R, E, N, B] =
    Edomaton(env =>
      run(env).flatMap { r =>
        r.result.visit(
          err => r.copy(result = Decision.Rejected(err)).pure[F],
          f(_)
            .run(env)
            .map(o =>
              o.result.visit(
                _ => o,
                _ => r >> o
              )
            )
        )
      }
    )

  inline def >>=[Env2 <: Env, B](
      f: A => Edomaton[F, Env2, R, E, N, B]
  )(using Monad[F]): Edomaton[F, Env2, R, E, N, B] = flatMap(f)

  inline def >>[Env2 <: Env, B](
      f: Edomaton[F, Env2, R, E, N, B]
  )(using Monad[F]): Edomaton[F, Env2, R, E, N, B] = andThen(f)

  inline def andThen[Env2 <: Env, B](
      f: Edomaton[F, Env2, R, E, N, B]
  )(using Monad[F]): Edomaton[F, Env2, R, E, N, B] = flatMap(_ => f)

  def transform[B](f: Response[R, E, N, A] => Response[R, E, N, B])(using
      Functor[F]
  ): Edomaton[F, Env, R, E, N, B] =
    Edomaton(run.andThen(_.map(f)))

  def mapK[G[_]](fk: FunctionK[F, G]): Edomaton[G, Env, R, E, N, A] =
    Edomaton(run.andThen(fk.apply))

  def evalMap[B](f: A => F[B])(using
      Monad[F]
  ): Edomaton[F, Env, R, E, N, B] =
    flatMap(a => liftF(f(a).map(Response.pure)))

  def evalTap[B](f: A => F[B])(using
      Monad[F]
  ): Edomaton[F, Env, R, E, N, A] =
    flatMap(a => liftF(f(a).map(Response.pure)).as(a))

  def eval[B](f: => F[B])(using Monad[F]): Edomaton[F, Env, R, E, N, A] =
    evalTap(_ => f)

  inline def as[B](b: B)(using Functor[F]): Edomaton[F, Env, R, E, N, B] =
    map(_ => b)

  /** Clears all notifications so far */
  def reset(using Functor[F]): Edomaton[F, Env, R, E, N, A] =
    transform(_.reset)

  /** Adds notification without considering decision state */
  def publish(ns: N*)(using Functor[F]): Edomaton[F, Env, R, E, N, A] =
    transform(_.publish(ns: _*))

  def publishOnRejectionWith(
      f: NonEmptyChain[R] => Seq[N]
  )(using Functor[F]): Edomaton[F, Env, R, E, N, A] = transform(
    _.publishOnRejectionWith(f)
  )

  def publishOnRejection(ns: N*)(using
      Functor[F]
  ): Edomaton[F, Env, R, E, N, A] =
    transform(_.publishOnRejection(ns: _*))
}

object Edomaton extends ServiceMonadInstances, ServiceMonadConstructors

sealed transparent trait ServiceMonadInstances {
  given [F[_]: Monad, Env, R, E, N]
      : Monad[[t] =>> Edomaton[F, Env, R, E, N, t]] =
    new Monad {
      type G[T] = Edomaton[F, Env, R, E, N, T]
      override def pure[A](x: A): G[A] =
        Edomaton(_ => Response.pure(x).pure[F])

      override def map[A, B](fa: G[A])(f: A => B): G[B] = fa.map(f)

      override def flatMap[A, B](fa: G[A])(f: A => G[B]): G[B] = fa.flatMap(f)

      override def tailRecM[A, B](a: A)(
          f: A => G[Either[A, B]]
      ): G[B] = Edomaton(env =>
        Monad[F].tailRecM(Response.pure[R, E, N, A](a))(rma =>
          rma.result.visit(
            _ => ???, // This cannot happen
            a =>
              f(a)
                .run(env)
                .map(rma >> _)
                .map(o =>
                  o.result
                    .visit(
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
      Eq[Env => F[Response[R, E, N, T]]]
  ): Eq[Edomaton[F, Env, R, E, N, T]] =
    Eq.by(_.run)

  given [F[_], R, E, N, T]
      : Contravariant[[env] =>> Edomaton[F, env, R, E, N, T]] =
    new Contravariant {
      override def contramap[A, B](fa: Edomaton[F, A, R, E, N, T])(
          f: B => A
      ): Edomaton[F, B, R, E, N, T] = fa.contramap(f)
    }

}

sealed transparent trait ServiceMonadConstructors {
  def pure[F[_]: Monad, Env, R, E, N, T](
      t: T
  ): Edomaton[F, Env, R, E, N, T] =
    Edomaton(_ => Response.pure(t).pure)

  def unit[F[_]: Monad, Env, R, E, N, T]: Edomaton[F, Env, R, E, N, Unit] =
    pure(())

  def liftF[F[_], Env, R, E, N, T](
      f: F[Response[R, E, N, T]]
  ): Edomaton[F, Env, R, E, N, T] = Edomaton(_ => f)

  def lift[F[_]: Applicative, Env, R, E, N, T](
      f: Response[R, E, N, T]
  ): Edomaton[F, Env, R, E, N, T] = liftF(f.pure)

  def eval[F[_]: Applicative, Env, R, E, N, T](
      f: F[T]
  ): Edomaton[F, Env, R, E, N, T] = liftF(f.map(Response.pure))

  def run[F[_]: Applicative, Env, R, E, N, T](
      f: Env => F[T]
  ): Edomaton[F, Env, R, E, N, T] = Edomaton(
    f.andThen(_.map(Response.pure))
  )

  def map[F[_]: Applicative, Env, R, E, N, T](
      f: Env => T
  ): Edomaton[F, Env, R, E, N, T] =
    run(f.andThen(_.pure))

  def read[F[_]: Applicative, Env, R, E, N, T]: Edomaton[F, Env, R, E, N, Env] =
    run(_.pure[F])

  def publish[F[_]: Applicative, Env, R, E, N](
      ns: N*
  ): Edomaton[F, Env, R, E, N, Unit] = lift(Response.publish(ns: _*))

  def reject[F[_]: Applicative, Env, R, E, N, T](
      r: R,
      rs: R*
  ): Edomaton[F, Env, R, E, N, T] = lift(Response.reject(r, rs: _*))

  def perform[F[_]: Applicative, Env, R, E, N, T](
      d: Decision[R, E, T]
  ): Edomaton[F, Env, R, E, N, T] = lift(Response.lift(d))

}
