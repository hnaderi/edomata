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

/** Represents programs that are event driven state machines (a Mealy machine)
  *
  * these programs can use input to decide on a state transition, and optionally
  * emit a sequence of notifications for communication.
  *
  * @tparam F
  *   effect type
  * @tparam Env
  *   input type
  * @tparam R
  *   rejection type
  * @tparam E
  *   internal event type
  * @tparam N
  *   notification type, a.k.a external event, integration event
  * @tparam A
  *   output type
  */
final case class Edomaton[F[_], -Env, R, E, N, A](
    run: Env => F[Response[R, E, N, A]]
) {

  /** maps output result */
  def map[B](f: A => B)(using Functor[F]): Edomaton[F, Env, R, E, N, B] =
    transform(_.map(f))

  /** creates a new edomaton that translates some input to what this edomaton
    * can understand
    */
  def contramap[Env2](f: Env2 => Env): Edomaton[F, Env2, R, E, N, A] =
    Edomaton(
      run.compose(f)
    )

  /** binds another edomaton to this one. */
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

  /** alias for flatMap */
  inline def >>=[Env2 <: Env, B](
      f: A => Edomaton[F, Env2, R, E, N, B]
  )(using Monad[F]): Edomaton[F, Env2, R, E, N, B] = flatMap(f)

  /** alias for andThen, flatMap(_ => f) */
  inline def >>[Env2 <: Env, B](
      f: => Edomaton[F, Env2, R, E, N, B]
  )(using Monad[F]): Edomaton[F, Env2, R, E, N, B] = andThen(f)

  /** sequences another edomaton without considering output for this one
    *
    * alias for flatMap(_=> f)
    */
  inline def andThen[Env2 <: Env, B](
      f: => Edomaton[F, Env2, R, E, N, B]
  )(using Monad[F]): Edomaton[F, Env2, R, E, N, B] = flatMap(_ => f)

  /** transforms underlying response
    */
  def transform[B](f: Response[R, E, N, A] => Response[R, E, N, B])(using
      Functor[F]
  ): Edomaton[F, Env, R, E, N, B] =
    Edomaton(run.andThen(_.map(f)))

  /** translates this program in another language mapped by a natural
    * transformation
    */
  def mapK[G[_]](fk: FunctionK[F, G]): Edomaton[G, Env, R, E, N, A] =
    Edomaton(run.andThen(fk.apply))

  /** evaluates an effect using output of this edomaton and uses result of
    * evaluation as new output
    */
  def evalMap[B](f: A => F[B])(using
      Monad[F]
  ): Edomaton[F, Env, R, E, N, B] =
    flatMap(a => liftF(f(a).map(Response.pure)))

  /** like evalMap but ignores evaluation result
    */
  def evalTap[B](f: A => F[B])(using
      Monad[F]
  ): Edomaton[F, Env, R, E, N, A] =
    flatMap(a => liftF(f(a).map(Response.pure)).as(a))

  /** evaluates an effect and uses its result as new output */
  def eval[B](f: => F[B])(using Monad[F]): Edomaton[F, Env, R, E, N, A] =
    evalTap(_ => f)

  /** alias for map(_=> b) */
  inline def as[B](b: B)(using Functor[F]): Edomaton[F, Env, R, E, N, B] =
    map(_ => b)

  /** Clears all notifications so far */
  def reset(using Functor[F]): Edomaton[F, Env, R, E, N, A] =
    transform(_.reset)

  /** Adds notification without considering decision state */
  def publish(ns: N*)(using Functor[F]): Edomaton[F, Env, R, E, N, A] =
    transform(_.publish(ns: _*))

  /** If this edomaton is rejected, uses given function to decide what to
    * publish
    */
  def publishOnRejectionWith(
      f: NonEmptyChain[R] => Seq[N]
  )(using Functor[F]): Edomaton[F, Env, R, E, N, A] = transform(
    _.publishOnRejectionWith(f)
  )

  /** publishes these notifications if this edomaton is rejected */
  def publishOnRejection(ns: N*)(using
      Functor[F]
  ): Edomaton[F, Env, R, E, N, A] =
    transform(_.publishOnRejection(ns: _*))
}

object Edomaton extends EdomatonInstances, EdomatonConstructors

sealed transparent trait EdomatonInstances {
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

sealed transparent trait EdomatonConstructors {

  /** constructs an edomaton that outputs a pure value */
  def pure[F[_]: Monad, Env, R, E, N, T](
      t: T
  ): Edomaton[F, Env, R, E, N, T] =
    Edomaton(_ => Response.pure(t).pure)

  /** an edomaton with trivial output */
  def unit[F[_]: Monad, Env, R, E, N, T]: Edomaton[F, Env, R, E, N, Unit] =
    pure(())

  /** constructs an edomaton from an effect that results in a response */
  def liftF[F[_], Env, R, E, N, T](
      f: F[Response[R, E, N, T]]
  ): Edomaton[F, Env, R, E, N, T] = Edomaton(_ => f)

  /** constructs an edomaton with given response */
  def lift[F[_]: Applicative, Env, R, E, N, T](
      f: Response[R, E, N, T]
  ): Edomaton[F, Env, R, E, N, T] = liftF(f.pure)

  /** constructs an edomaton that evaluates an effect */
  def eval[F[_]: Applicative, Env, R, E, N, T](
      f: F[T]
  ): Edomaton[F, Env, R, E, N, T] = liftF(f.map(Response.pure))

  /** constructs an edomaton that runs an effect using its input */
  def run[F[_]: Applicative, Env, R, E, N, T](
      f: Env => F[T]
  ): Edomaton[F, Env, R, E, N, T] = Edomaton(
    f.andThen(_.map(Response.pure))
  )

  /** constructs an edomaton that outputs what's read */
  def read[F[_]: Applicative, Env, R, E, N, T]: Edomaton[F, Env, R, E, N, Env] =
    run(_.pure[F])

  /** constructs an edomaton that publishes given notifications */
  def publish[F[_]: Applicative, Env, R, E, N](
      ns: N*
  ): Edomaton[F, Env, R, E, N, Unit] = lift(Response.publish(ns: _*))

  /** constructs an edomaton that rejects with given rejections */
  def reject[F[_]: Applicative, Env, R, E, N, T](
      r: R,
      rs: R*
  ): Edomaton[F, Env, R, E, N, T] = lift(Response.reject(r, rs: _*))

  /** constructs an edomaton that decides the given decision */
  def decide[F[_]: Applicative, Env, R, E, N, T](
      d: Decision[R, E, T]
  ): Edomaton[F, Env, R, E, N, T] = lift(Response.lift(d))

}
