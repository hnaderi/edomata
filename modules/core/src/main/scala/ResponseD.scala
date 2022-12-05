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

type ResponseD[+R, +E, +N, +A] = ResponseT[Decision[R, E, *], R, N, A]
object ResponseD {
  def apply[R, E, N, A](
      result: Decision[R, E, A],
      notifications: Chain[N] = Chain.nil
  ): ResponseD[R, E, N, A] = ResponseT(result, notifications)

  def unapply[R, E, N, A](
      d: ResponseD[R, E, N, A]
  ): Some[(Decision[R, E, A], Chain[N])] = Some((d.result, d.notifications))

  /** constructs a program that outputs a pure value */
  def pure[T](t: T): ResponseD[Nothing, Nothing, Nothing, T] = ResponseT(
    Decision(t)
  )

  /** a program with trivial output */
  val unit: ResponseD[Nothing, Nothing, Nothing, Unit] = pure(())

  /** constructs a program that publishes given notifications */
  def publish[N](n: N*): ResponseD[Nothing, Nothing, N, Unit] =
    ResponseT(Decision.unit, Chain.fromSeq(n))

  /** Constructs a program that decides to accept a sequence of events */
  def accept[E](ev: E, evs: E*): ResponseD[Nothing, E, Nothing, Unit] =
    acceptReturn(())(ev, evs: _*)

  /** Constructs a program that decides to accept a sequence of events and also
    * returns an output
    */
  def acceptReturn[E, T](
      t: T
  )(ev: E, evs: E*): ResponseD[Nothing, E, Nothing, T] =
    apply(Decision.Accepted(NonEmptyChain.of(ev, evs: _*), t))

  /** constructs a program that rejects with given rejections */
  def reject[R](
      reason: R,
      otherReasons: R*
  ): ResponseD[R, Nothing, Nothing, Nothing] =
    reject(NonEmptyChain.of(reason, otherReasons: _*))

  def reject[R](
      reasons: NonEmptyChain[R]
  ): ResponseD[R, Nothing, Nothing, Nothing] =
    ResponseT(Decision.Rejected(reasons))

  /** Constructs a program that uses a validation to decide whether to output a
    * value or reject with error(s)
    */
  def validate[R, T](
      validation: ValidatedNec[R, T]
  ): ResponseD[R, Nothing, Nothing, T] =
    validation.fold(reject(_), pure(_))

  /** constructs a program with given decision */
  def validate[R, T](d: EitherNec[R, T]): ResponseD[R, Nothing, Nothing, T] =
    d.fold(reject(_), pure(_))
}
