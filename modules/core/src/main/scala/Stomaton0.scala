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

import Stomaton0.*

/** Represents programs that are event driven state machines (a Mealy machine)
  *
  * these programs can use input to decide to emit events and change states.
  * these programs are not event sourced and change state directly.
  *
  * @tparam F
  *   effect type
  * @tparam Env
  *   input type
  * @tparam S
  *   state type used for making decisions
  * @tparam R
  *   rejection type
  * @tparam E
  *   event type
  * @tparam A
  *   output type
  */
final case class Stomaton0[F[_], Env, S, R, E, A](
    run: (Env, S) => DecisionT[F, R, E, (S, A)]
) extends AnyVal {

  /** maps output result */
  def map[B](f: A => B)(using Functor[F]): Stomaton0[F, Env, S, R, E, B] =
    Stomaton0((env, state) => run(env, state).map((s, a) => (s, f(a))))

  /** binds another stomaton to this one. */
  def flatMap[Env2 <: Env, B](
      f: A => Stomaton0[F, Env2, S, R, E, B]
  )(using Monad[F]): Stomaton0[F, Env2, S, R, E, B] =
    Stomaton0((env, state) =>
      run(env, state).flatMap { (newState, a) =>
        f(a).run(env, newState)
      }
    )

  inline def >>=[Env2 <: Env, B](
      f: A => Stomaton0[F, Env2, S, R, E, B]
  )(using Monad[F]): Stomaton0[F, Env2, S, R, E, B] = flatMap(f)

  inline def >>[Env2 <: Env, B](
      f: => Stomaton0[F, Env2, S, R, E, B]
  )(using Monad[F]): Stomaton0[F, Env2, S, R, E, B] = andThen(f)

  inline def andThen[Env2 <: Env, B](
      f: => Stomaton0[F, Env2, S, R, E, B]
  )(using Monad[F]): Stomaton0[F, Env2, S, R, E, B] = flatMap(_ => f)

  /** creates a new stomaton that translates some input to what this stomaton
    * can understand
    */
  def contramap[Env2](f: Env2 => Env): Stomaton0[F, Env2, S, R, E, A] =
    Stomaton0((env, state) => run(f(env), state))

  def modify(f: S => S)(using Functor[F]): Stomaton0[F, Env, S, R, E, A] =
    Stomaton0((env, s0) => run(env, s0).map((s1, t) => (f(s1), t)))

  def decideS(
      f: S => Decision[R, E, S]
  )(using Monad[F]): Stomaton0[F, Env, S, R, E, S] =
    Stomaton0((env, s0) =>
      run(env, s0).flatMapD((s1, t) => f(s1).map(ns => (ns, ns)))
    )

  def decide[B](
      f: A => Decision[R, E, B]
  )(using Monad[F]): Stomaton0[F, Env, S, R, E, B] =
    Stomaton0((env, s0) =>
      run(env, s0).flatMapD((s1, t) => f(t).map(b => (s1, b)))
    )

  def set(s: S)(using Functor[F]): Stomaton0[F, Env, S, R, E, A] =
    Stomaton0((env, s0) => run(env, s0).map((_, t) => (s, t)))

  def handleErrorWith(
      f: NonEmptyChain[R] => Stomaton0[F, Env, S, R, E, A]
  )(using Monad[F]): Stomaton0[F, Env, S, R, E, A] = Stomaton0((env, state) =>
    DecisionT(
      runF(env, state).flatMap {
        case Decision.Rejected(err) => f(err).runF(env, state)
        case other                  => other.pure
      }
    )
  )

  inline def runF: (Env, S) => F[Decision[R, E, (S, A)]] = run(_, _).run
}

object Stomaton0 extends StomatonInstances, StomatonConstructors

sealed transparent trait StomatonInstances {
  given [F[_]: Monad, Env, S, R, E]
      : MonadError[[t] =>> Stomaton0[F, Env, S, R, E, t], NonEmptyChain[R]] =
    new MonadError {

      override def raiseError[A](
          e: NonEmptyChain[R]
      ): Stomaton0[F, Env, S, R, E, A] =
        Stomaton0.decide(Decision.Rejected(e))

      override def handleErrorWith[A](fa: Stomaton0[F, Env, S, R, E, A])(
          f: NonEmptyChain[R] => Stomaton0[F, Env, S, R, E, A]
      ): Stomaton0[F, Env, S, R, E, A] = fa.handleErrorWith(f)

      type G[T] = Stomaton0[F, Env, S, R, E, T]
      type D[T] = DecisionT[F, R, E, T]
      override def pure[A](x: A): G[A] =
        Stomaton0((_, s) => DecisionT.pure((s, x)))

      override def map[A, B](fa: G[A])(f: A => B): G[B] = fa.map(f)

      override def flatMap[A, B](fa: G[A])(f: A => G[B]): G[B] = fa.flatMap(f)

      override def tailRecM[A, B](a: A)(
          f: A => G[Either[A, B]]
      ): G[B] =
        Stomaton0((env, s0) =>
          (s0, a).tailRecM((state, value) =>
            f(value).run(env, state).map {
              case (newState, Right(b)) => Right((newState, b))
              case (newState, Left(a))  => Left(newState, a)
            }
          )
        )
    }

  given [F[_], Env, S, R, E, T](using
      Eq[(Env, S) => DecisionT[F, R, E, (S, T)]]
  ): Eq[Stomaton0[F, Env, S, R, E, T]] =
    Eq.by(_.run)

  given [F[_], S, R, E, T]
      : Contravariant[[env] =>> Stomaton0[F, env, S, R, E, T]] =
    new Contravariant {
      override def contramap[A, B](fa: Stomaton0[F, A, S, R, E, T])(
          f: B => A
      ): Stomaton0[F, B, S, R, E, T] = fa.contramap(f)
    }

}

sealed transparent trait StomatonConstructors {

  /** constructs an stomaton that outputs a pure value */
  def pure[F[_]: Applicative, Env, S, R, E, T](
      t: T
  ): Stomaton0[F, Env, S, R, E, T] =
    Stomaton0((_, s) => DecisionT.pure((s, t)))

  /** an stomaton with trivial output */
  def unit[F[_]: Applicative, Env, R, E, N, T]
      : Stomaton0[F, Env, R, E, N, Unit] =
    pure(())

  /** constructs an stomaton with given response */
  def lift[F[_]: Applicative, Env, S, R, E, T](
      f: DecisionT[F, R, E, T]
  ): Stomaton0[F, Env, S, R, E, T] = Stomaton0((_, s) => f.map((s, _)))

  /** constructs an stomaton that decides the given decision */
  def decide[F[_]: Applicative, Env, S, R, E, T](
      f: Decision[R, E, T]
  ): Stomaton0[F, Env, S, R, E, T] = lift(DecisionT.lift(f))

  /** constructs an stomaton that evaluates an effect */
  def eval[F[_]: Applicative, Env, S, R, E, T](
      f: F[T]
  ): Stomaton0[F, Env, S, R, E, T] =
    Stomaton0((_, s) => DecisionT.liftF(f).map((s, _)))

  /** constructs an stomaton that runs an effect using its input */
  def run[F[_]: Applicative, Env, S, R, E, T](
      f: Env => F[T]
  ): Stomaton0[F, Env, S, R, E, T] =
    Stomaton0((env, state) => DecisionT.liftF(f(env)).map((state, _)))

  /** constructs an stomaton that outputs the context */
  def context[F[_]: Applicative, Env, S, R, E, T]
      : Stomaton0[F, Env, S, R, E, Env] =
    run(_.pure[F])

  /** constructs an stomaton that outputs the current state */
  def state[F[_]: Applicative, Env, S, R, E]: Stomaton0[F, Env, S, R, E, S] =
    Stomaton0((_, s) => DecisionT.pure((s, s)))

  /** constructs an stomaton that sets the current state */
  def set[F[_]: Applicative, Env, S, R, E](
      s: S
  ): Stomaton0[F, Env, S, R, E, Unit] =
    Stomaton0((_, _) => DecisionT.pure((s, ())))

  /** constructs an stomaton that modifies current state */
  def modify[F[_]: Applicative, Env, S, R, E](
      f: S => S
  ): Stomaton0[F, Env, S, R, E, S] =
    Stomaton0((_, s) =>
      val ns = f(s)
      DecisionT.pure((ns, ns))
    )

  /** constructs an stomaton that decides to modify state based on current state
    */
  def modifyS[F[_]: Applicative, Env, S, R, E](
      f: S => Decision[R, E, S]
  ): Stomaton0[F, Env, S, R, E, S] =
    Stomaton0((_, s) => DecisionT.lift(f(s).map(ns => (ns, ns))))

  /** constructs an stomaton that rejects with given rejections */
  def reject[F[_]: Applicative, Env, S, R, E, T](
      r: R,
      rs: R*
  ): Stomaton0[F, Env, S, R, E, T] = decide(Decision.reject(r, rs: _*))

  def validate[F[_]: Applicative, Env, S, R, E, T](
      v: ValidatedNec[R, T]
  ): Stomaton0[F, Env, S, R, E, T] = decide(Decision.validate(v))

  /** Constructs a program from an optional value, that outputs value if exists
    * or rejects otherwise
    *
    * You can also use .toDecision syntax for more convenience
    */
  def fromOption[F[_]: Applicative, Env, S, R, E, T](
      opt: Option[T],
      orElse: R,
      other: R*
  ): Stomaton0[F, Env, S, R, E, T] =
    opt.fold(reject(orElse, other: _*))(pure(_))

  /** Constructs a program that either outputs a value or rejects
    */
  def fromEither[F[_]: Applicative, Env, S, R, E, T](
      eit: Either[R, T]
  ): Stomaton0[F, Env, S, R, E, T] = eit.fold(reject(_), pure(_))

  /** Constructs a program that either outputs a value or rejects
    *
    * You can also use .toDecision syntax for more convenience.
    */
  def fromEitherNec[F[_]: Applicative, Env, S, R, E, T](
      eit: EitherNec[R, T]
  ): Stomaton0[F, Env, S, R, E, T] =
    eit.fold(errs => decide(Decision.Rejected(errs)), pure(_))
}
