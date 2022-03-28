package edomata.core

import cats.Applicative
import cats.Functor
import cats.Monad
import cats.data.ValidatedNec
import cats.implicits._

final case class Response[N, T](
    result: T,
    notifications: Seq[N] = Nil
)

final class RequestMonad[F[_], -Env, N, A](
    val run: Env => F[Response[N, A]]
) extends Serializable {

  def map[B](
      f: A => B
  )(using F: Functor[F]): RequestMonad[F, Env, N, B] =
    RequestMonad(env =>
      F.map(run(env))(out => out.copy(result = f(out.result)))
    )

  def flatMap[Env2 <: Env, N2 >: N, B](
      f: A => RequestMonad[F, Env2, N2, B]
  )(using M: Monad[F]): RequestMonad[F, Env2, N2, B] =
    RequestMonad { env =>
      M.flatMap(run(env)) { case Response(a, notifs) =>
        f(a)
          .run(env)
          .map(out => out.copy(notifications = notifs ++ out.notifications))
      }
    }

  def as[B](b: B)(using F: Functor[F]): RequestMonad[F, Env, N, B] =
    map(_ => b)

  /** Clears all notifications so far */
  def reset(using F: Functor[F]): RequestMonad[F, Env, N, A] =
    RequestMonad(run.andThen(_.map(_.copy(notifications = Nil))))

  /** Adds notification without considering decision state */
  def publish(ns: N*)(using F: Functor[F]): RequestMonad[F, Env, N, A] =
    RequestMonad(
      run.andThen(
        _.map(res => res.copy(notifications = res.notifications ++ ns))
      )
    )
}

object RequestMonad
    extends RequestMonadConstructors
    with RequestMonadCatsInstances

sealed transparent trait RequestMonadConstructors {
  def ask[F[_], Env, N](using F: Applicative[F]): RequestMonad[F, Env, N, Env] =
    RequestMonad(e => Response(e).pure[F])

  def lift[F[_], Env, N, T](
      t: Response[N, T]
  )(using F: Applicative[F]): RequestMonad[F, Env, N, T] =
    RequestMonad(_ => F.pure(t))

  def pure[F[_], Env, N, T](
      t: T
  )(using F: Applicative[F]): RequestMonad[F, Env, N, T] =
    lift(Response(t))

  def void[F[_]: Applicative, Env, N]: RequestMonad[F, Env, N, Unit] = pure(())

  def liftF[F[_], Env, N, T](
      f: F[T]
  )(using F: Functor[F]): RequestMonad[F, Env, N, T] =
    RequestMonad(_ => F.map(f)(Response(_)))

  def publish[F[_]: Applicative, Env, N](
      ns: N*
  ): RequestMonad[F, Env, N, Unit] =
    lift(Response(Decision.unit, ns))
}

sealed transparent trait RequestMonadCatsInstances {
  type DT[F[_], Env, N] = [T] =>> RequestMonad[F, Env, N, T]

  given [F[_], Env, N](using F: Functor[F]): Functor[DT[F, Env, N]] with
    def map[A, B](fa: RequestMonad[F, Env, N, A])(
        f: A => B
    ): RequestMonad[F, Env, N, B] = fa.map(f)

  given [F[_], Env, N](using F: Monad[F]): Monad[DT[F, Env, N]] with
    override def map[A, B](fa: RequestMonad[F, Env, N, A])(
        f: A => B
    ): RequestMonad[F, Env, N, B] = fa.map(f)

    def flatMap[A, B](fa: RequestMonad[F, Env, N, A])(
        f: A => RequestMonad[F, Env, N, B]
    ): RequestMonad[F, Env, N, B] = fa.flatMap(f)

    def tailRecM[A, B](
        a: A
    )(
        f: A => RequestMonad[F, Env, N, Either[A, B]]
    ): RequestMonad[F, Env, N, B] =
      RequestMonad { env =>
        F.tailRecM(a) { a =>
          f(a).run(env).map { res =>
            res.result.map(b => res.copy(result = b))
          }
        }
      }

    def pure[A](x: A): RequestMonad[F, Env, N, A] =
      RequestMonad.pure(x)

}
