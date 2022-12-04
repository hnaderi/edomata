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
import cats.data.*
import cats.implicits.*
import cats.kernel.Eq

import scala.annotation.tailrec

/** Representation of programs that decide and emit notifications
  *
  * This adds capability of emiting notifications/integration events to
  * [[Decision]] programs
  *
  * @tparam R
  *   rejection type
  * @tparam E
  *   domain event type
  * @tparam N
  *   notification type
  * @tparam A
  *   output type
  */
final case class ResponseT[RES[+_, +_], +R, +N, +A](
    result: RES[R, A],
    notifications: Chain[N] = Chain.nil
) {
  def map[R2 >: R, B](f: A => B)(using
      F: Functor[RES[R2, *]]
  ): ResponseT[RES, R2, N, B] =
    copy(result = F.map(result)(f))

  def flatMap[R2 >: R, N2 >: N, B](
      f: A => ResponseT[RES, R2, N2, B]
  )(using
      F: MonadError[RES[R2, *], NonEmptyChain[R2]],
      E: RaiseError[RES]
  ): ResponseT[RES, R2, N2, B] =
    E.toEither(result) match {
      case Right(a) =>
        val out = f(a)
        val res = F.flatMap(result)(_ => out.result)
        if E.isError(res) then ResponseT(res, out.notifications)
        else ResponseT(res, notifications ++ out.notifications)
      case Left(err) => ResponseT(F.raiseError(err), notifications)
    }

  inline def as[R2 >: R, B](b: B)(using
      Functor[RES[R2, *]]
  ): ResponseT[RES, R2, N, B] =
    map(_ => b)

  inline def >>[R2 >: R, N2 >: N, B](
      f: A => ResponseT[RES, R2, N2, B]
  )(using
      MonadError[RES[R2, *], NonEmptyChain[R2]],
      RaiseError[RES]
  ): ResponseT[RES, R2, N2, B] = flatMap(f)

  /** Clears all notifications so far */
  def reset: ResponseT[RES, R, N, A] =
    copy(notifications = Chain.nil)

  /** Adds notification without considering decision state */
  def publish[N2 >: N](ns: N2*): ResponseT[RES, R, N2, A] =
    copy(notifications = notifications ++ Chain.fromSeq(ns))

  /** If it is rejected, uses given function to decide what to publish
    */
  def publishOnRejectionWith[N2 >: N](
      f: NonEmptyChain[R] => Seq[N2]
  )(using E: RaiseError[RES]): ResponseT[RES, R, N2, A] = publish(
    E.fold(result)(f, _ => Nil): _*
  )

  /** publishes these notifications if this program result is rejected */
  def publishOnRejection[N2 >: N](ns: N2*)(using
      E: RaiseError[RES]
  ): ResponseT[RES, R, N2, A] =
    publishOnRejectionWith(_ => ns)

  def handleErrorWith[R2 >: R, N2 >: N, A2 >: A](
      f: NonEmptyChain[R] => ResponseT[RES, R2, N2, A2]
  )(using E: RaiseError[RES]): ResponseT[RES, R2, N2, A2] =
    E.fold(result)(f, _ => this)
}

object ResponseT extends ResponseTConstructors with ResponseTCatsInstances0

sealed trait ResponseTConstructors {

  /** constructs a program that outputs a pure value */
  def pure[RES[+_, +_], T](t: T)(using
      F: Monad[RES[Nothing, *]]
  ): ResponseT[RES, Nothing, Nothing, T] = ResponseT(F.pure(t))

  /** a program with trivial output */
  def unit[RES[+_, +_], R](using
      Monad[RES[Nothing, *]]
  ): ResponseT[RES, Nothing, Nothing, Unit] = pure(())

  /** constructs a program with given decision */
  def lift[RES[+_, +_], R, T](d: RES[R, T]): ResponseT[RES, R, Nothing, T] =
    ResponseT(d)

  /** constructs a program that publishes given notifications */
  def publish[RES[+_, +_], R, N](n: N*)(using
      F: Monad[RES[R, *]]
  ): ResponseT[RES, R, N, Unit] =
    ResponseT(F.unit, Chain.fromSeq(n))

  /** constructs a program that rejects with given rejections */
  def reject[RES[+_, +_], R, N](
      reason: R,
      otherReasons: R*
  )(using
      F: MonadError[RES[R, *], NonEmptyChain[R]]
  ): ResponseT[RES, R, N, Nothing] =
    ResponseT(F.raiseError(NonEmptyChain.of(reason, otherReasons: _*)))

  /** Constructs a program that uses a validation to decide whether to output a
    * value or reject with error(s)
    */
  def validate[RES[+_, +_], R, T](
      validation: ValidatedNec[R, T]
  )(using
      F: MonadError[RES[R, *], NonEmptyChain[R]]
  ): ResponseT[RES, R, Nothing, T] =
    ResponseT(validation.fold(F.raiseError[T](_), F.pure(_)))
}

sealed trait ResponseTCatsInstances0 extends ResponseTCatsInstances1 {
  given [RES[+_, +_], R, N](using
      F: MonadError[RES[R, *], NonEmptyChain[R]],
      E: RaiseError[RES]
  ): MonadError[[t] =>> ResponseT[RES, R, N, t], NonEmptyChain[R]] =
    new MonadError {

      override def raiseError[A](e: NonEmptyChain[R]): ResponseT[RES, R, N, A] =
        ResponseT(F.raiseError(e))

      override def handleErrorWith[A](fa: ResponseT[RES, R, N, A])(
          f: NonEmptyChain[R] => ResponseT[RES, R, N, A]
      ): ResponseT[RES, R, N, A] =
        fa.handleErrorWith(f)

      override def map[A, B](fa: ResponseT[RES, R, N, A])(
          f: A => B
      ): ResponseT[RES, R, N, B] = fa.map(f)

      def flatMap[A, B](fa: ResponseT[RES, R, N, A])(
          f: A => ResponseT[RES, R, N, B]
      ): ResponseT[RES, R, N, B] = fa.flatMap(f)

      def tailRecM[A, B](
          a: A
      )(
          f: A => ResponseT[RES, R, N, Either[A, B]]
      ): ResponseT[RES, R, N, B] = {
        @tailrec
        def loop(
            value: ResponseT[RES, R, N, Either[A, B]]
        ): ResponseT[RES, R, N, B] = {
          E.toEither(value.result) match {
            case Right(Right(b)) =>
              value.copy(value.result.as(b))
            case Right(Left(a)) =>
              val na = f(a)
              val res = value.result >> na.result
              val ns =
                if E.isError(res) then na.notifications
                else value.notifications ++ na.notifications
              loop(ResponseT(res, ns))
            case Left(err) => value.copy(F.raiseError(err))
          }
        }

        loop(f(a))
      }

      def pure[A](x: A): ResponseT[RES, R, N, A] =
        ResponseT(F.pure(x))
    }

  given [RES[+_, +_], R, N, T]: Eq[ResponseT[RES, R, N, T]] =
    Eq.instance(_ == _)
}

sealed trait ResponseTCatsInstances1 {
  given [RES[+_, +_], R, N](using
      Traverse[RES[R, *]]
  ): Traverse[[t] =>> ResponseT[RES, R, N, t]] = new Traverse {
    def traverse[G[_]: Applicative, A, B](fa: ResponseT[RES, R, N, A])(
        f: A => G[B]
    ): G[ResponseT[RES, R, N, B]] =
      fa.result.traverse(f).map(b => fa.copy(result = b))

    def foldLeft[A, B](fa: ResponseT[RES, R, N, A], b: B)(f: (B, A) => B): B =
      fa.result.foldLeft(b)(f)

    def foldRight[A, B](fa: ResponseT[RES, R, N, A], lb: Eval[B])(
        f: (A, Eval[B]) => Eval[B]
    ): Eval[B] =
      fa.result.foldRight(lb)(f)
  }
}

private[core] transparent trait ResponseTConstructorsO[Res[+_, +_], App[
    +R,
    +N,
    +A
] >: ResponseT[Res, R, N, A]](using
    M: RaiseError[Res]
) {
  def apply[R, E, A](
      result: Res[R, A],
      notifications: Chain[E] = Chain.nil
  ): App[R, E, A] = ResponseT(result, notifications)

  /** constructs a program that outputs a pure value */
  def pure[T](t: T): App[Nothing, Nothing, T] = ResponseT(M.pure(t))

  /** a program with trivial output */
  val unit: App[Nothing, Nothing, Unit] = pure(())

  /** constructs a program that publishes given notifications */
  def publish[N](n: N*): App[Nothing, N, Unit] =
    ResponseT(M.unit, Chain.fromSeq(n))

  /** constructs a program that rejects with given rejections */
  def reject[R](
      reason: R,
      otherReasons: R*
  ): App[R, Nothing, Nothing] =
    reject(NonEmptyChain.of(reason, otherReasons: _*))

  def reject[R](
      reasons: NonEmptyChain[R]
  ): App[R, Nothing, Nothing] =
    ResponseT(M.raise(reasons))

  /** Constructs a program that uses a validation to decide whether to output a
    * value or reject with error(s)
    */
  def validate[R, N, T](
      validation: ValidatedNec[R, T]
  ): App[R, N, T] =
    validation.fold(reject(_), pure(_))

  /** constructs a program with given decision */
  def validate[R, T](d: EitherNec[R, T]): App[R, Nothing, T] =
    d.fold(reject(_), pure(_))
}
