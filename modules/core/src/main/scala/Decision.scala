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
import cats.Eval
import cats.MonadError
import cats.Traverse
import cats.data.Chain
import cats.data.EitherNec
import cats.data.NonEmptyChain
import cats.data.Validated
import cats.data.ValidatedNec
import cats.kernel.Eq

import scala.annotation.tailrec
import scala.annotation.targetName
import scala.util.NotGiven

import Decision._

/** Represents programs that decide in an event driven context
  *
  * This is basically a simple state machine like the following:
  * ```
  * [*] -> InDecisive
  * InDecisive -- event --> Accepted
  * InDecisive -- join --> InDecisive
  * InDecisive -- rejection --> Rejected (resets and terminates)
  * Accepted -- event --> Accepted (accumulates)
  * Accepted -- rejection --> Rejected (resets and terminates)
  * ```
  *
  * It forms a monad error and also is traversable.
  *
  * @tparam R
  *   rejection type
  * @tparam E
  *   event type
  * @tparam A
  *   program output type
  */
sealed trait Decision[+R, +E, +A] extends Product with Serializable { self =>

  /** creates a new decision that changes the output value of this one */
  def map[B](f: A => B): Decision[R, E, B] =
    self match {
      case InDecisive(t)     => InDecisive(f(t))
      case Accepted(evs, a)  => Accepted(evs, f(a))
      case Rejected(reasons) => Rejected(reasons)
    }

  /** binds another decision to this one, creates a new decision */
  def flatMap[R2 >: R, E2 >: E, B](
      f: A => Decision[R2, E2, B]
  ): Decision[R2, E2, B] =
    self match {
      case Accepted(events, result) =>
        f(result) match {
          case Accepted(events2, result) =>
            Accepted(events ++ events2, result)
          case InDecisive(result) => Accepted(events, result)
          case other              => other
        }
      case InDecisive(result) => f(result)
      case Rejected(reasons)  => Rejected(reasons)
    }

  inline def >>=[R2 >: R, E2 >: E, B](
      f: A => Decision[R2, E2, B]
  ): Decision[R2, E2, B] = flatMap(f)

  inline def >>[R2 >: R, E2 >: E, B](
      f: => Decision[R2, E2, B]
  ): Decision[R2, E2, B] = flatMap(_ => f)

  def as[B](b: B): Decision[R, E, B] = map(_ => b)

  def flatTap[R2 >: R, E2 >: E, B](
      f: A => Decision[R2, E2, B]
  ): Decision[R2, E2, A] = flatMap(a => f(a).as(a))

  /** whether is rejected or not */
  def isRejected: Boolean = self match {
    case Rejected(_) => true
    case _           => false
  }

  /** whether is accepted or not */
  def isAccepted: Boolean = self match {
    case Accepted(_, _) => true
    case _              => false
  }

  /** traverses this decision, run fr if there are errors and runs fa if there
    * is some output
    */
  def visit[B](fr: NonEmptyChain[R] => B, fa: A => B): B = self match {
    case Decision.InDecisive(a)  => fa(a)
    case Decision.Accepted(_, a) => fa(a)
    case Decision.Rejected(r)    => fr(r)
  }

  /** Ignores events and creates a ValidatedNec */
  def toValidated: ValidatedNec[R, A] = self match {
    case Decision.Accepted(_, t) => Validated.Valid(t)
    case Decision.InDecisive(t)  => Validated.Valid(t)
    case Decision.Rejected(r)    => Validated.Invalid(r)
  }

  /** Ignores events and errors and creates an Option that contains program
    * output
    */
  def toOption: Option[A] = visit(_ => None, Some(_))

  /** Ignores events and creates an Either */
  def toEither: EitherNec[R, A] = visit(Left(_), Right(_))

  /** Ignores output value */
  def void: Decision[R, E, Unit] = map(_ => ())

  /** Validates output using a ValidatedNec */
  def validate[R2 >: R, B](f: A => ValidatedNec[R2, B]): Decision[R2, E, B] =
    flatMap(a => Decision.validate(f(a)))

  /** Validates output using an EitherNec */
  @targetName("validateEitherNec")
  def validate[R2 >: R, B](f: A => EitherNec[R2, B]): Decision[R2, E, B] =
    flatMap(a => Decision.fromEitherNec(f(a)))

  /** Validates output using an Either */
  @targetName("validateEither")
  def validate[R2 >: R, B](f: A => Either[R2, B]): Decision[R2, E, B] =
    flatMap(a => Decision.fromEither(f(a)))

  /** Asserts output using a ValidatedNec without changing it */
  def assert[R2 >: R, B](f: A => ValidatedNec[R2, B]): Decision[R2, E, A] =
    flatTap(a => Decision.validate(f(a)))

  /** Asserts output using an EitherNec without changing it */
  @targetName("assertEitherNec")
  def assert[R2 >: R, B](f: A => EitherNec[R2, B]): Decision[R2, E, A] =
    flatTap(a => Decision.fromEitherNec(f(a)))

  /** Asserts output using an Either without changing it */
  @targetName("assertEither")
  def assert[R2 >: R, B](f: A => Either[R2, B]): Decision[R2, E, A] =
    flatTap(a => Decision.fromEither(f(a)))

}

object Decision extends DecisionConstructors, DecisionCatsInstances0 {
  final case class InDecisive[T](result: T)
      extends Decision[Nothing, Nothing, T]
  final case class Accepted[E, T](events: NonEmptyChain[E], result: T)
      extends Decision[Nothing, E, T]
  final case class Rejected[R](reasons: NonEmptyChain[R])
      extends Decision[R, Nothing, Nothing]
}

sealed trait DecisionConstructors {

  /** Constructs a program that outputs a pure value */
  def apply[T](t: T): Decision[Nothing, Nothing, T] = InDecisive(t)

  /** Constructs a program that outputs a pure value */
  def pure[T](t: T): Decision[Nothing, Nothing, T] = InDecisive(t)

  /** Constructs a program that outputs a trivial output */
  def unit: Decision[Nothing, Nothing, Unit] = InDecisive(())

  /** Constructs a program that decides to accept a sequence of events */
  def accept[E](ev: E, evs: E*): Decision[Nothing, E, Unit] =
    acceptReturn(())(ev, evs: _*)

  /** Constructs a program that decides to accept a sequence of events and also
    * returns an output
    */
  def acceptReturn[E, T](t: T)(ev: E, evs: E*): Decision[Nothing, E, T] =
    Accepted(NonEmptyChain.of(ev, evs: _*), t)

  /** Constructs a program that decides to accept a sequence of events or do
    * nothing based on a boolean predicate
    */
  def acceptWhen[E](
      predicate: => Boolean
  )(ev: E, evs: E*): Decision[Nothing, E, Unit] =
    if predicate then accept(ev, evs: _*)
    else unit

  /** Constructs a program that decides to accept an optional single event or do
    * nothing
    */
  def acceptWhen[E](ev: Option[E]): Decision[Nothing, E, Unit] =
    ev.fold(unit)(accept(_))

  /** Constructs a program that decides to accept an optional single event or
    * reject otherwise
    */
  def acceptWhen[R, E](
      ev: Option[E]
  )(orElse: R, other: R*): Decision[R, E, Unit] =
    ev.fold(reject(orElse, other: _*))(accept(_))

  /** Constructs a program that decides to accept an optional single event or
    * reject otherwise
    */
  def acceptWhen[R, E](
      eit: Either[R, E]
  ): Decision[R, E, Unit] = eit.fold(reject(_), accept(_))

  /** Constructs a program that decides to accept an optional single event or
    * reject otherwise
    */
  @targetName("acceptWhenEitherNec")
  def acceptWhen[R, E](
      eit: EitherNec[R, E]
  ): Decision[R, E, Unit] = eit.fold(Rejected(_), accept(_))

  /** Constructs a program that decides to reject with a sequence of reasons */
  def reject[R, E](
      reason: R,
      otherReasons: R*
  ): Decision[R, E, Nothing] =
    Rejected(NonEmptyChain.of(reason, otherReasons: _*))

  /** Constructs a program that decides to reject with a sequence of reasons if
    * a predicate satisfies
    */
  def rejectWhen[R](
      predicate: => Boolean
  )(ev: R, evs: R*): Decision[R, Nothing, Unit] =
    if predicate then reject(ev, evs: _*)
    else unit

  /** Constructs a program that decides to reject with an optional single reason
    * or do nothing otherwise
    */
  def rejectWhen[R](ev: Option[R]): Decision[R, Nothing, Unit] =
    ev.fold(unit)(reject(_))

  /** Constructs a program that uses a validation to decide whether to output a
    * value or reject with error(s)
    *
    * You can also use .toDecision syntax for more convenience
    */
  def validate[R, T](
      validation: ValidatedNec[R, T]
  ): Decision[R, Nothing, T] =
    validation match {
      case Validated.Invalid(e) => Rejected(e)
      case Validated.Valid(a)   => InDecisive(a)
    }

  /** Constructs a program from an optional value, that outputs value if exists
    * or rejects otherwise
    *
    * You can also use .toDecision syntax for more convenience
    */
  def fromOption[R, T](
      opt: Option[T],
      orElse: R,
      other: R*
  ): Decision[R, Nothing, T] = opt.fold(reject(orElse, other: _*))(pure(_))

  /** Constructs a program that either outputs a value or rejects
    */
  def fromEither[R, T](
      eit: Either[R, T]
  ): Decision[R, Nothing, T] = eit.fold(reject(_), pure(_))

  /** Constructs a program that either outputs a value or rejects
    *
    * You can also use .toDecision syntax for more convenience.
    */
  def fromEitherNec[R, T](
      eit: EitherNec[R, T]
  ): Decision[R, Nothing, T] = eit.fold(Rejected(_), pure(_))
}

private type D[R, E] = [T] =>> Decision[R, E, T]

sealed trait DecisionCatsInstances0 extends DecisionCatsInstances1 {
  given [R, E]: MonadError[D[R, E], NonEmptyChain[R]] =
    new MonadError[D[R, E], NonEmptyChain[R]] {
      override def pure[A](x: A): Decision[R, E, A] = Decision.pure(x)

      override def map[A, B](
          fa: Decision[R, E, A]
      )(f: A => B): Decision[R, E, B] =
        fa.map(f)

      override def flatMap[A, B](fa: Decision[R, E, A])(
          f: A => Decision[R, E, B]
      ): Decision[R, E, B] =
        fa.flatMap(f)

      @tailrec
      private def step[A, B](a: A, evs: Chain[E] = Chain.empty)(
          f: A => Decision[R, E, Either[A, B]]
      ): Decision[R, E, B] =
        f(a) match {
          case Decision.Accepted(ev2, Left(a)) =>
            step(a, evs ++ ev2.toChain)(f)
          case Decision.Accepted(ev2, Right(b)) =>
            Decision.Accepted(ev2.prependChain(evs), b)
          case Decision.InDecisive(Left(a)) => step(a, evs)(f)
          case Decision.InDecisive(Right(b)) =>
            NonEmptyChain
              .fromChain(evs)
              .fold(Decision.InDecisive(b))(Decision.Accepted(_, b))
          case Decision.Rejected(rs) => Decision.Rejected(rs)
        }

      override def tailRecM[A, B](a: A)(
          f: A => Decision[R, E, Either[A, B]]
      ): Decision[R, E, B] =
        step(a)(f)

      def handleErrorWith[A](
          fa: Decision[R, E, A]
      )(f: NonEmptyChain[R] => Decision[R, E, A]): Decision[R, E, A] =
        fa match {
          case Decision.Rejected(e) => f(e)
          case other                => other
        }

      def raiseError[A](e: NonEmptyChain[R]): Decision[R, E, A] =
        Decision.Rejected(e)

    }

  given [R, E, T]: Eq[Decision[R, E, T]] = Eq.instance(_ == _)
}

sealed trait DecisionCatsInstances1 {

  given [R, E]: Traverse[D[R, E]] = new Traverse {
    def traverse[G[_]: Applicative, A, B](fa: Decision[R, E, A])(
        f: A => G[B]
    ): G[Decision[R, E, B]] = fa.visit(
      rej => Applicative[G].pure(Decision.Rejected(rej)),
      a => Applicative[G].map(f(a))(b => fa.map(_ => b))
    )

    def foldLeft[A, B](fa: Decision[R, E, A], b: B)(f: (B, A) => B): B =
      fa.visit(_ => b, a => f(b, a))

    def foldRight[A, B](fa: Decision[R, E, A], lb: Eval[B])(
        f: (A, Eval[B]) => Eval[B]
    ): Eval[B] =
      fa.visit(_ => lb, a => f(a, lb))
  }
}

private[edomata] transparent trait DecisionSyntax {
  extension [R, T](self: ValidatedNec[R, T]) {
    def toDecision[E]: Decision[R, E, T] = Decision.validate(self)
  }

  private type NonChain[R] = NotGiven[R <:< NonEmptyChain[Nothing]]

  extension [T](self: Option[T]) {
    def toDecision[R](orElse: R, others: R*): Decision[R, Nothing, T] =
      Decision.fromOption(self, orElse, others: _*)
    def toAccepted: Decision[Nothing, T, Unit] = Decision.acceptWhen(self)
    def toAcceptedOr[R](orElse: R, others: R*): Decision[R, T, Unit] =
      Decision.acceptWhen(self)(orElse, others: _*)
    def toRejected: Decision[T, Nothing, Unit] = Decision.rejectWhen(self)
  }

  extension [R, T](self: EitherNec[R, T]) {
    def toDecision: Decision[R, Nothing, T] =
      Decision.fromEitherNec(self)
    def toAccepted: Decision[R, T, Unit] = Decision.acceptWhen(self)
  }

  extension [R, T](self: Either[R, T])(using NonChain[R]) {
    def toDecision: Decision[R, Nothing, T] =
      Decision.fromEither(self)
    def toAccepted: Decision[R, T, Unit] = Decision.acceptWhen(self)
  }

  extension [T](value: T) {
    def asDecision: Decision[Nothing, Nothing, T] = Decision.pure(value)
    def accept: Decision[Nothing, T, Unit] = Decision.accept(value)
    def reject: Decision[T, Nothing, Nothing] = Decision.reject(value)
  }
}
