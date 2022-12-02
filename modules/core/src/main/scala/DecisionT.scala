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

import cats.Applicative
import cats.Functor
import cats.Monad
import cats.MonadError
import cats.data.Chain
import cats.data.NonEmptyChain
import cats.data.ValidatedNec
import cats.implicits._
import cats.kernel.Eq
import edomata.core.Decision.Accepted
import edomata.core.Decision.InDecisive
import edomata.core.Decision.Rejected

/** This is monad transformer for [[Decision]]
  *
  * @tparam F
  *   effect type
  * @tparam R
  *   rejection type
  * @tparam E
  *   event type
  * @tparam A
  *   program output type
  */
final case class DecisionT[F[_], R, E, A](run: F[Decision[R, E, A]]) {
  def map[B](f: A => B)(using F: Functor[F]): DecisionT[F, R, E, B] =
    DecisionT(F.map(run)(_.map(f)))

  def flatMap[R2 >: R, E2 >: E, B](
      f: A => DecisionT[F, R2, E2, B]
  )(using M: Monad[F]): DecisionT[F, R2, E2, B] =
    DecisionT {
      M.flatMap(run) {
        case e1 @ Accepted(events1, result) =>
          f(result).run.map {
            case e2 @ Accepted(events2, _) =>
              e2.copy(events = events1 ++ events2)
            case InDecisive(result) => e1.copy(result = result)
            case r @ Rejected(_)    => r.copy()
          }
        case InDecisive(result) => f(result).run
        case r @ Rejected(_)    => M.pure(r.copy())
      }
    }

  def flatMapD[R2 >: R, E2 >: E, B](
      f: A => Decision[R2, E2, B]
  )(using M: Monad[F]): DecisionT[F, R2, E2, B] =
    flatMap(f.andThen(DecisionT.lift))

  def as[B](b: B)(using F: Functor[F]): DecisionT[F, R, E, B] = map(_ => b)
}

object DecisionT extends DecisionTConstructors with DecisionTCatsInstances {
  type Of[F[_], R, E] = [t] =>> DecisionT[F, R, E, t]
}

sealed transparent trait DecisionTConstructors {

  /** Constructs a program that outputs a pure value */
  def pure[F[_], R, E, T](
      t: T
  )(using F: Applicative[F]): DecisionT[F, R, E, T] =
    DecisionT(F.pure(Decision.pure(t)))

  /** Constructs a program that outputs a trivial output */
  def unit[F[_]: Applicative, R, E]: DecisionT[F, R, E, Unit] = pure(())

  /** Lifts a Decision to DecisionT */
  def lift[F[_], R, E, T](
      t: Decision[R, E, T]
  )(using F: Applicative[F]): DecisionT[F, R, E, T] =
    DecisionT(F.pure(t))

  /** Lifts an effect to DecisionT */
  def liftF[F[_], R, E, T](
      f: F[T]
  )(using F: Functor[F]): DecisionT[F, R, E, T] =
    DecisionT(F.map(f)(Decision.pure))

  /** Constructs a program that uses a validation to decide whether to output a
    * value or reject with error(s)
    */
  def validate[F[_]: Applicative, R, E, T](
      validation: ValidatedNec[R, T]
  ): DecisionT[F, R, E, T] =
    lift(Decision.validate(validation))

  /** Constructs a program that decides to accept a sequence of events */
  def accept[F[_]: Applicative, R, E](
      ev: E,
      evs: E*
  ): DecisionT[F, R, E, Unit] =
    lift(Decision.accept(ev, evs: _*))

  /** Constructs a program that decides to accept a sequence of events and also
    * returns an output
    */
  def acceptReturn[F[_]: Applicative, R, E, T](t: T)(
      ev: E,
      evs: E*
  ): DecisionT[F, R, E, T] =
    lift(Decision.acceptReturn(t)(ev, evs: _*))

  /** Constructs a program that decides to reject with a sequence of reasons */
  def reject[F[_]: Applicative, R, E](
      reason: R,
      otherReasons: R*
  ): DecisionT[F, R, E, Nothing] =
    lift(Decision.reject(reason, otherReasons: _*))
}

sealed transparent trait DecisionTCatsInstances {
  type DT[F[_], R, E] = [T] =>> DecisionT[F, R, E, T]

  given [F[_], R, E](using F: Functor[F]): Functor[DT[F, R, E]] with
    def map[A, B](fa: DecisionT[F, R, E, A])(
        f: A => B
    ): DecisionT[F, R, E, B] = fa.map(f)

  given [F[_], R, E](using
      F: Monad[F]
  ): MonadError[DT[F, R, E], NonEmptyChain[R]] with
    override def map[A, B](fa: DecisionT[F, R, E, A])(
        f: A => B
    ): DecisionT[F, R, E, B] = fa.map(f)

    def flatMap[A, B](fa: DecisionT[F, R, E, A])(
        f: A => DecisionT[F, R, E, B]
    ): DecisionT[F, R, E, B] = fa.flatMap(f)

    def tailRecM[A, B](
        a: A
    )(f: A => DecisionT[F, R, E, Either[A, B]]): DecisionT[F, R, E, B] =
      DecisionT {
        F.tailRecM((a, Chain.empty[E])) { (a, evs) =>
          f(a).run.map {
            case e @ Accepted(events, result) =>
              result.fold(
                a => Left((a, evs ++ events.toChain)),
                b => Right(Accepted(events.prependChain(evs), b))
              )
            case e @ InDecisive(result) =>
              result.fold(
                a => Left((a, evs)),
                b =>
                  NonEmptyChain
                    .fromChain(evs)
                    .fold(InDecisive(b))(Accepted(_, b))
                    .asRight
              )
            case e @ Rejected(reasons) => Right(Rejected(reasons))
          }
        }
      }

    def pure[A](x: A): DecisionT[F, R, E, A] =
      DecisionT.pure(x)

    def raiseError[A](e: NonEmptyChain[R]): DecisionT[F, R, E, A] =
      DecisionT.lift(Decision.Rejected(e))

    def handleErrorWith[A](fa: DecisionT[F, R, E, A])(
        f: NonEmptyChain[R] => DecisionT[F, R, E, A]
    ): DecisionT[F, R, E, A] = DecisionT {
      F.flatMap(fa.run) {
        case Decision.Rejected(e) => f(e).run
        case other                => F.pure(other)
      }
    }

  given [F[_], R, E, A](using
      Eq[F[Decision[R, E, A]]]
  ): Eq[DecisionT[F, R, E, A]] = Eq.by(_.run)
}
