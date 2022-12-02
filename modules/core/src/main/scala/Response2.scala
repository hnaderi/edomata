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
final case class Response2[+R, +N, +A](
    result: EitherNec[R, A],
    notifications: Chain[N] = Chain.nil
) {
  def map[B](f: A => B): Response2[R, N, B] =
    copy(result = result.map(f))

  def flatMap[R2 >: R, N2 >: N, B](
      f: A => Response2[R2, N2, B]
  ): Response2[R2, N2, B] = result match {
    case Left(rej) => Response2(Left(rej), notifications)
    case Right(a) =>
      val out = f(a)
      out.result match {
        case Left(rej) => Response2(Left(rej), out.notifications)
        case other     => Response2(other, notifications ++ out.notifications)
      }
  }

  inline def as[B](b: B): Response2[R, N, B] = map(_ => b)

  inline def >>[R2 >: R, N2 >: N, B](
      f: A => Response2[R2, N2, B]
  ): Response2[R2, N2, B] = flatMap(f)

  /** Clears all notifications so far */
  def reset: Response2[R, N, A] =
    copy(notifications = Chain.nil)

  /** Adds notification without considering decision state */
  def publish[N2 >: N](ns: N2*): Response2[R, N2, A] =
    copy(notifications = notifications ++ Chain.fromSeq(ns))

  /** If it is rejected, uses given function to decide what to publish
    */
  def publishOnRejectionWith[N2 >: N](
      f: NonEmptyChain[R] => Seq[N2]
  ): Response2[R, N2, A] = publish(result.fold(f, _ => Nil): _*)

  /** publishes these notifications if this program result is rejected */
  def publishOnRejection[N2 >: N](ns: N2*): Response2[R, N2, A] =
    publishOnRejectionWith(_ => ns)
}

object Response2 extends Response2Constructors with Response2CatsInstances0

sealed trait Response2Constructors {

  /** constructs a program that outputs a pure value */
  def pure[R, N, T](t: T): Response2[R, N, T] = Response2(Right(t))

  /** a program with trivial output */
  def unit[R, N]: Response2[R, N, Unit] = pure(())

  /** constructs a program with given decision */
  def lift[R, N, T](d: EitherNec[R, T]): Response2[R, N, T] =
    Response2(d)

  /** constructs a program that publishes given notifications */
  def publish[R, N](n: N*): Response2[R, N, Unit] =
    Response2(Either.unit, Chain.fromSeq(n))

  /** constructs a program that rejects with given rejections */
  def reject[R, N](
      reason: R,
      otherReasons: R*
  ): Response2[R, N, Nothing] =
    Response2(Left(NonEmptyChain.of(reason, otherReasons: _*)))

  /** Constructs a program that uses a validation to decide whether to output a
    * value or reject with error(s)
    */
  def validate[R, N, T](
      validation: ValidatedNec[R, T]
  ): Response2[R, N, T] =
    Response2(validation.toEither)
}

sealed trait Response2CatsInstances0 extends Response2CatsInstances1 {
  given [R, N]: Monad[[t] =>> Response2[R, N, t]] =
    new Monad {
      override def map[A, B](fa: Response2[R, N, A])(
          f: A => B
      ): Response2[R, N, B] = fa.map(f)

      def flatMap[A, B](fa: Response2[R, N, A])(
          f: A => Response2[R, N, B]
      ): Response2[R, N, B] = fa.flatMap(f)

      @tailrec
      private def step[A, B](
          a: A,
          ns: Chain[N] = Chain.nil
      )(
          f: A => Response2[R, N, Either[A, B]]
      ): Response2[R, N, B] =
        val out = f(a)
        out.result match {
          case Left(errs)      => out.copy(result = Left(errs))
          case Right(Right(b)) => Response2(Right(b), ns ++ out.notifications)
          case Right(Left(a))  => step(a, ns ++ out.notifications)(f)
        }

      def tailRecM[A, B](
          a: A
      )(
          f: A => Response2[R, N, Either[A, B]]
      ): Response2[R, N, B] =
        step(a)(f)

      def pure[A](x: A): Response2[R, N, A] =
        Response2(Right(x))
    }

  given [R, N, T]: Eq[Response2[R, N, T]] = Eq.instance(_ == _)
}

sealed trait Response2CatsInstances1 {
  given [R, N]: Traverse[[t] =>> Response2[R, N, t]] = new Traverse {
    def traverse[G[_]: Applicative, A, B](fa: Response2[R, N, A])(
        f: A => G[B]
    ): G[Response2[R, N, B]] =
      fa.result.traverse(f).map(b => fa.copy(result = b))

    def foldLeft[A, B](fa: Response2[R, N, A], b: B)(f: (B, A) => B): B =
      fa.result.foldLeft(b)(f)

    def foldRight[A, B](fa: Response2[R, N, A], lb: Eval[B])(
        f: (A, Eval[B]) => Eval[B]
    ): Eval[B] =
      fa.result.foldRight(lb)(f)
  }
}
