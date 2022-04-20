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
import cats.Functor
import cats.Monad
import cats.MonadError
import cats.Traverse
import cats.data.Chain
import cats.data.NonEmptyChain
import cats.data.ValidatedNec
import cats.implicits.*
import cats.kernel.Eq

import scala.annotation.tailrec

final case class Response[+R, +E, +N, +A](
    result: Decision[R, E, A],
    notifications: Chain[N] = Chain.nil
) {
  def map[B](f: A => B): Response[R, E, N, B] =
    copy(result = result.map(f))

  def flatMap[R2 >: R, E2 >: E, N2 >: N, B](
      f: A => Response[R2, E2, N2, B]
  ): Response[R2, E2, N2, B] = result.visit(
    rej => Response(Decision.Rejected(rej), notifications),
    a => {
      val out = f(a)
      result.flatMap(_ => out.result) match {
        case d @ Decision.Rejected(_) => Response(d, out.notifications)
        case other => Response(other, notifications ++ out.notifications)
      }
    }
  )

  inline def as[B](b: B): Response[R, E, N, B] = map(_ => b)

  inline def >>[R2 >: R, E2 >: E, N2 >: N, B](
      f: A => Response[R2, E2, N2, B]
  ): Response[R2, E2, N2, B] = flatMap(f)

  /** Clears all notifications so far */
  def reset: Response[R, E, N, A] =
    copy(notifications = Chain.nil)

  /** Adds notification without considering decision state */
  def publish[N2 >: N](ns: N2*): Response[R, E, N2, A] =
    copy(notifications = notifications ++ Chain.fromSeq(ns))

  def publishOnRejectionWith[N2 >: N](
      f: NonEmptyChain[R] => Seq[N2]
  ): Response[R, E, N2, A] = publish(result.visit(f, _ => Nil): _*)

  def publishOnRejection[N2 >: N](ns: N2*): Response[R, E, N2, A] =
    publishOnRejectionWith(_ => ns)
}

object Response extends ResponseConstructors with ResponseCatsInstances0

sealed trait ResponseConstructors {
  def pure[R, E, N, T](t: T): Response[R, E, N, T] = Response(
    Decision.pure(t)
  )

  def unit[R, E, N]: Response[R, E, N, Unit] = pure(())

  def lift[R, E, N, T](d: Decision[R, E, T]): Response[R, E, N, T] =
    Response(d)

  def publish[R, E, N](n: N*): Response[R, E, N, Unit] =
    Response(Decision.unit, Chain.fromSeq(n))

  def accept[R, E, N](ev: E, evs: E*): Response[R, E, N, Unit] =
    acceptReturn(())(ev, evs: _*)

  def acceptReturn[R, E, N, T](
      t: T
  )(ev: E, evs: E*): Response[R, E, N, T] =
    Response(Decision.Accepted(NonEmptyChain.of(ev, evs: _*), t))

  def reject[R, E, N](
      reason: R,
      otherReasons: R*
  ): Response[R, E, N, Nothing] =
    Response(Decision.Rejected(NonEmptyChain.of(reason, otherReasons: _*)))

  def validate[R, E, N, T](
      validation: ValidatedNec[R, T]
  ): Response[R, E, N, T] =
    Response(Decision.validate(validation))
}

sealed trait ResponseCatsInstances0 extends ResponseCatsInstances1 {
  given [R, E, N]: Monad[[t] =>> Response[R, E, N, t]] =
    new Monad {
      override def map[A, B](fa: Response[R, E, N, A])(
          f: A => B
      ): Response[R, E, N, B] = fa.map(f)

      def flatMap[A, B](fa: Response[R, E, N, A])(
          f: A => Response[R, E, N, B]
      ): Response[R, E, N, B] = fa.flatMap(f)

      @tailrec
      private def step[A, B](
          a: A,
          evs: Chain[E] = Chain.nil,
          ns: Chain[N] = Chain.nil
      )(
          f: A => Response[R, E, N, Either[A, B]]
      ): Response[R, E, N, B] =
        val out = f(a)
        out.result match {
          case Decision.Accepted(ev2, e) =>
            e match {
              case Left(a) =>
                step(a, evs ++ ev2.toChain, ns ++ out.notifications)(f)
              case Right(b) =>
                Response(
                  Decision.Accepted(ev2.prependChain(evs), b),
                  ns ++ out.notifications
                )
            }
          case Decision.InDecisive(e) =>
            e match {
              case Left(a) =>
                step(a, evs, ns ++ out.notifications)(f)
              case Right(b) =>
                Response(
                  NonEmptyChain
                    .fromChain(evs)
                    .fold(Decision.InDecisive(b))(Decision.Accepted(_, b)),
                  ns ++ out.notifications
                )
            }
          case Decision.Rejected(rs) =>
            out.copy(result = Decision.Rejected(rs))
        }

      def tailRecM[A, B](
          a: A
      )(
          f: A => Response[R, E, N, Either[A, B]]
      ): Response[R, E, N, B] =
        step(a)(f)

      def pure[A](x: A): Response[R, E, N, A] =
        Response(Decision.pure(x))
    }

  given [R, E, N, T]: Eq[Response[R, E, N, T]] = Eq.instance(_ == _)
}

sealed trait ResponseCatsInstances1 {
  given [R, E, N]: Traverse[[t] =>> Response[R, E, N, t]] = new Traverse {
    def traverse[G[_]: Applicative, A, B](fa: Response[R, E, N, A])(
        f: A => G[B]
    ): G[Response[R, E, N, B]] =
      fa.result.traverse(f).map(b => fa.copy(result = b))

    def foldLeft[A, B](fa: Response[R, E, N, A], b: B)(f: (B, A) => B): B =
      fa.result.foldLeft(b)(f)

    def foldRight[A, B](fa: Response[R, E, N, A], lb: Eval[B])(
        f: (A, Eval[B]) => Eval[B]
    ): Eval[B] =
      fa.result.foldRight(lb)(f)
  }
}
