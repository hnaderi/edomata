/*
 * Copyright 2021 Hossein Naderi
 *
 * Licensed under the Apache License, Version 2.0 (the "License");
 * you may not use this file except in compliance with the License.
 * You may obtain a copy of the License at
 *
 *     http://www.apache.org/licenses/LICENSE-2.0
 *
 * Unless required by applicable law or agreed to in writing, software
 * distributed under the License is distributed on an "AS IS" BASIS,
 * WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
 * See the License for the specific language governing permissions and
 * limitations under the License.
 */

package edomata.core

import cats.*
import cats.arrow.FunctionK
import cats.data.*
import cats.implicits.*
import cats.kernel.Eq
import Stomaton.*

final case class Stomaton2[F[_], Env, S, R, E, A](
    run: (Env, S) => F[Response2[R, E, (S, A)]]
) extends AnyVal {

  /** maps output result */
  def map[B](f: A => B)(using Functor[F]): Stomaton2[F, Env, S, R, E, B] =
    Stomaton2((env, state) => run(env, state).map(_.map((s, a) => (s, f(a)))))

  /** binds another stomaton to this one. */
  def flatMap[Env2 <: Env, B](
      f: A => Stomaton2[F, Env2, S, R, E, B]
  )(using Monad[F]): Stomaton2[F, Env2, S, R, E, B] =
    Stomaton2((env, state) =>
      run(env, state).flatMap { out =>
        out.result match {
          case Right((newState, a)) =>
            f(a)
              .run(env, newState)
              .map(o =>
                o.copy(notifications = out.notifications ++ o.notifications)
              )
          case Left(errs) => out.copy(result = Left(errs)).pure
        }
      }
    )

  inline def >>=[Env2 <: Env, B](
      f: A => Stomaton2[F, Env2, S, R, E, B]
  )(using Monad[F]): Stomaton2[F, Env2, S, R, E, B] = flatMap(f)

  inline def >>[Env2 <: Env, B](
      f: => Stomaton2[F, Env2, S, R, E, B]
  )(using Monad[F]): Stomaton2[F, Env2, S, R, E, B] = andThen(f)

  inline def andThen[Env2 <: Env, B](
      f: => Stomaton2[F, Env2, S, R, E, B]
  )(using Monad[F]): Stomaton2[F, Env2, S, R, E, B] = flatMap(_ => f)

  /** creates a new stomaton that translates some input to what this stomaton
    * can understand
    */
  def contramap[Env2](f: Env2 => Env): Stomaton2[F, Env2, S, R, E, A] =
    Stomaton2((env, state) => run(f(env), state))

  def modify(f: S => S)(using Functor[F]): Stomaton2[F, Env, S, R, E, A] =
    Stomaton2((env, s0) => run(env, s0).map(_.map((s1, t) => (f(s1), t))))

  def decideS(
      f: S => EitherNec[R, S]
  )(using Monad[F]): Stomaton2[F, Env, S, R, E, S] =
    Stomaton2((env, s0) =>
      run(env, s0).map(res =>
        res.flatMap((ns, _) => Response2(f(ns).map(s => (s, s))))
      )
    )

  def decide[B](
      f: A => EitherNec[R, B]
  )(using Monad[F]): Stomaton2[F, Env, S, R, E, B] =
    Stomaton2((env, s0) =>
      run(env, s0).map(res =>
        res.flatMap((ns, a) => Response2(f(a).map(b => (ns, b))))
      )
    )

  def set(s: S)(using Functor[F]): Stomaton2[F, Env, S, R, E, A] =
    Stomaton2((env, s0) => run(env, s0).map(_.map((_, a) => (s, a))))

  def handleErrorWith(
      f: NonEmptyChain[R] => Stomaton2[F, Env, S, R, E, A]
  )(using Monad[F]): Stomaton2[F, Env, S, R, E, A] = Stomaton2((env, state) =>
    run(env, state).flatMap { out =>
      out.result.fold(
        f(_).run(env, state),
        _ => out.pure[F]
      )
    }
  )
}

object Stomaton2 extends Stomaton2Instances, Stomaton2Constructors

sealed transparent trait Stomaton2Instances {
  given [F[_]: Monad, Env, S, R, E]
      : MonadError[[t] =>> Stomaton2[F, Env, S, R, E, t], NonEmptyChain[R]] =
    new MonadError {

      override def raiseError[A](
          e: NonEmptyChain[R]
      ): Stomaton2[F, Env, S, R, E, A] =
        Stomaton2.decide(Left(e))

      override def handleErrorWith[A](fa: Stomaton2[F, Env, S, R, E, A])(
          f: NonEmptyChain[R] => Stomaton2[F, Env, S, R, E, A]
      ): Stomaton2[F, Env, S, R, E, A] = fa.handleErrorWith(f)

      type G[T] = Stomaton2[F, Env, S, R, E, T]
      type D[T] = DecisionT[F, R, E, T]
      override def pure[A](x: A): G[A] =
        Stomaton2.pure(x)

      override def map[A, B](fa: G[A])(f: A => B): G[B] = fa.map(f)

      override def flatMap[A, B](fa: G[A])(f: A => G[B]): G[B] = fa.flatMap(f)

      override def tailRecM[A, B](a: A)(
          f: A => G[Either[A, B]]
      ): G[B] =
        Stomaton2((env, s0) =>
          Monad[F].tailRecM(Response2.pure[R, E, (S, A)]((s0, a))) { res =>
            res.result.fold(
              _ => ???,
              (s, a) =>
                f(a)
                  .run(env, s)
                  .map(res >> _)
                  .map(o =>
                    o.result match {
                      case Right((newState, Left(a))) =>
                        o.as((newState, a)).asLeft
                      case Right((newState, Right(b))) =>
                        o.as((newState, b)).asRight
                      case Left(errs) => o.copy(result = Left(errs)).asRight
                    }
                  )
            )
          }
        )
    }

  given [F[_], Env, S, R, E, T](using
      Eq[(Env, S) => F[Response2[R, E, (S, T)]]]
  ): Eq[Stomaton2[F, Env, S, R, E, T]] =
    Eq.by(_.run)

  given [F[_], S, R, E, T]
      : Contravariant[[env] =>> Stomaton2[F, env, S, R, E, T]] =
    new Contravariant {
      override def contramap[A, B](fa: Stomaton2[F, A, S, R, E, T])(
          f: B => A
      ): Stomaton2[F, B, S, R, E, T] = fa.contramap(f)
    }

}

sealed transparent trait Stomaton2Constructors {

  /** constructs an stomaton that outputs a pure value */
  def pure[F[_]: Applicative, Env, S, R, E, T](
      t: T
  ): Stomaton2[F, Env, S, R, E, T] =
    Stomaton2((_, s) => Response2.pure((s, t)).pure)

  /** an stomaton with trivial output */
  def unit[F[_]: Applicative, Env, R, E, N, T]
      : Stomaton2[F, Env, R, E, N, Unit] =
    pure(())

  /** constructs an stomaton that evaluates an effect */
  def eval[F[_]: Applicative, Env, S, R, E, T](
      f: F[T]
  ): Stomaton2[F, Env, S, R, E, T] =
    Stomaton2((_, s) => f.map((s, _).pure))

  /** constructs an stomaton that runs an effect using its input */
  def run[F[_]: Applicative, Env, S, R, E, T](
      f: Env => F[T]
  ): Stomaton2[F, Env, S, R, E, T] =
    Stomaton2((env, state) => f(env).map((state, _).pure))

  /** constructs an stomaton that outputs the context */
  def context[F[_]: Applicative, Env, S, R, E, T]
      : Stomaton2[F, Env, S, R, E, Env] =
    run(_.pure[F])

  /** constructs an stomaton that outputs the current state */
  def state[F[_]: Applicative, Env, S, R, E]: Stomaton2[F, Env, S, R, E, S] =
    Stomaton2((_, s) => Response2.pure((s, s)).pure)

  /** constructs an stomaton that sets the current state */
  def set[F[_]: Applicative, Env, S, R, E](
      s: S
  ): Stomaton2[F, Env, S, R, E, Unit] =
    Stomaton2((_, _) => Response2.pure((s, ())).pure)

  /** constructs an stomaton that modifies current state */
  def modify[F[_]: Applicative, Env, S, R, E](
      f: S => S
  ): Stomaton2[F, Env, S, R, E, S] =
    Stomaton2((_, s) =>
      val ns = f(s)
      Response2.pure((ns, ns)).pure
    )

  def decideS[F[_]: Applicative, Env, S, R, E](
      f: S => EitherNec[R, S]
  ): Stomaton2[F, Env, S, R, E, S] =
    Stomaton2((env, s0) => Response2(f(s0).map(s => (s, s))).pure)

  def decide[F[_]: Applicative, Env, S, R, E, T](
      f: => EitherNec[R, T]
  ): Stomaton2[F, Env, S, R, E, T] =
    Stomaton2((_, s) => Response2(f.map(t => (s, t))).pure)

  /** constructs an stomaton that decides to modify state based on current state
    */
  def modifyS[F[_]: Applicative, Env, S, R, E](
      f: S => EitherNec[R, S]
  ): Stomaton2[F, Env, S, R, E, S] =
    Stomaton2((_, s) => Response2.lift(f(s).map(ns => (ns, ns))).pure)

  /** constructs an stomaton that rejects with given rejections */
  def reject[F[_]: Applicative, Env, S, R, E, T](
      r: R,
      rs: R*
  ): Stomaton2[F, Env, S, R, E, T] = decide(NonEmptyChain(r, rs: _*).asLeft)

  def validate[F[_]: Applicative, Env, S, R, E, T](
      v: ValidatedNec[R, T]
  ): Stomaton2[F, Env, S, R, E, T] = decide(v.toEither)

  /** Constructs a program from an optional value, that outputs value if exists
    * or rejects otherwise
    *
    * You can also use .toDecision syntax for more convenience
    */
  def fromOption[F[_]: Applicative, Env, S, R, E, T](
      opt: Option[T],
      orElse: R,
      other: R*
  ): Stomaton2[F, Env, S, R, E, T] =
    opt.fold(reject(orElse, other: _*))(pure(_))

  /** Constructs a program that either outputs a value or rejects
    */
  def fromEither[F[_]: Applicative, Env, S, R, E, T](
      eit: Either[R, T]
  ): Stomaton2[F, Env, S, R, E, T] = eit.fold(reject(_), pure(_))

  /** Constructs a program that either outputs a value or rejects
    *
    * You can also use .toDecision syntax for more convenience.
    */
  def fromEitherNec[F[_]: Applicative, Env, S, R, E, T](
      eit: EitherNec[R, T]
  ): Stomaton2[F, Env, S, R, E, T] = decide(eit)

}
