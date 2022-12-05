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
final case class ResponseT[+RES[+_], +R, +N, +A](
    result: RES[A],
    notifications: Chain[N] = Chain.nil
) {
  def map[RES2[+x] >: RES[x], B](f: A => B)(using
      F: Functor[RES2]
  ): ResponseT[RES2, R, N, B] =
    copy(result = F.map(result)(f))

  def flatMap[RES2[+x] >: RES[x], R2 >: R, N2 >: N, B](
      f: A => ResponseT[RES2, R2, N2, B]
  )(using
      F: MonadError[RES2, NonEmptyChain[R2]],
      E: RaiseError[RES2, R2]
  ): ResponseT[RES2, R2, N2, B] =
    E.toEither(result) match {
      case Right(a) =>
        val out = f(a)
        val res = F.flatMap(result)(_ => out.result)
        if E.isError(res) then ResponseT(res, out.notifications)
        else ResponseT(res, notifications ++ out.notifications)
      case Left(err) => ResponseT(F.raiseError(err), notifications)
    }

  inline def as[RES2[+x] >: RES[x], B](b: B)(using
      Functor[RES2]
  ): ResponseT[RES2, R, N, B] =
    map(_ => b)

  inline def >>[RES2[+x] >: RES[x], R2 >: R, N2 >: N, B](
      f: A => ResponseT[RES2, R2, N2, B]
  )(using
      MonadError[RES2, NonEmptyChain[R2]],
      RaiseError[RES2, R2]
  ): ResponseT[RES2, R2, N2, B] = flatMap(f)

  /** Clears all notifications so far */
  def reset: ResponseT[RES, R, N, A] =
    copy(notifications = Chain.nil)

  /** Adds notification without considering decision state */
  def publish[N2 >: N](ns: N2*): ResponseT[RES, R, N2, A] =
    copy(notifications = notifications ++ Chain.fromSeq(ns))

  /** If it is rejected, uses given function to decide what to publish
    */
  def publishOnRejectionWith[RES2[+x] >: RES[x], R2 >: R, N2 >: N](
      f: NonEmptyChain[R2] => Seq[N2]
  )(using E: RaiseError[RES2, R2]): ResponseT[RES2, R, N2, A] = publish(
    E.fold(result)(f, _ => Nil): _*
  )

  /** publishes these notifications if this program result is rejected */
  def publishOnRejection[RES2[+x] >: RES[x], R2 >: R, N2 >: N](ns: N2*)(using
      E: RaiseError[RES2, R2]
  ): ResponseT[RES2, R2, N2, A] =
    publishOnRejectionWith[RES2, R2, N2](_ => ns)

  def handleErrorWith[RES2[+x] >: RES[x], R2 >: R, N2 >: N, A2 >: A](
      f: NonEmptyChain[R2] => ResponseT[RES2, R2, N2, A2]
  )(using E: RaiseError[RES2, R2]): ResponseT[RES2, R2, N2, A2] =
    E.fold(result)(f, _ => this)
}

object ResponseT extends ResponseT2Constructors with ResponseT2CatsInstances0

sealed trait ResponseT2Constructors {

  /** constructs a program that outputs a pure value */
  def pure[RES[+_], T](t: T)(using
      F: Monad[RES]
  ): ResponseT[RES, Nothing, Nothing, T] = ResponseT(F.pure(t))

  /** a program with trivial output */
  def unit[RES[+_], R](using
      Monad[RES]
  ): ResponseT[RES, Nothing, Nothing, Unit] = pure(())

  /** constructs a program with given decision */
  def lift[RES[+_], R, T](d: RES[T]): ResponseT[RES, R, Nothing, T] =
    ResponseT(d)

  /** constructs a program that publishes given notifications */
  def publish[RES[+_], R, N](n: N*)(using
      F: Monad[RES]
  ): ResponseT[RES, R, N, Unit] =
    ResponseT(F.unit, Chain.fromSeq(n))

  /** constructs a program that rejects with given rejections */
  def reject[RES[+_], R, N](
      reason: R,
      otherReasons: R*
  )(using
      F: MonadError[RES, NonEmptyChain[R]]
  ): ResponseT[RES, R, N, Nothing] =
    ResponseT(F.raiseError(NonEmptyChain.of(reason, otherReasons: _*)))

  /** Constructs a program that uses a validation to decide whether to output a
    * value or reject with error(s)
    */
  def validate[RES[+_], R, T](
      validation: ValidatedNec[R, T]
  )(using
      F: MonadError[RES, NonEmptyChain[R]]
  ): ResponseT[RES, R, Nothing, T] =
    ResponseT(validation.fold(F.raiseError[T](_), F.pure(_)))
}

sealed trait ResponseT2CatsInstances0 extends ResponseT2CatsInstances1 {
  given [RES[+_], R, N](using
      F: MonadError[RES, NonEmptyChain[R]],
      E: RaiseError[RES, R]
  ): MonadError[[t] =>> ResponseT[RES, R, N, t], NonEmptyChain[R]] =
    new MonadError {

      override def raiseError[A](
          e: NonEmptyChain[R]
      ): ResponseT[RES, R, N, A] =
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

  given [RES[+_], R, N, T]: Eq[ResponseT[RES, R, N, T]] =
    Eq.instance(_ == _)
}

sealed trait ResponseT2CatsInstances1 {
  given [RES[+_], R, N](using
      Traverse[RES]
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
