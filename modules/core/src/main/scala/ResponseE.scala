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
type ResponseE[+R, +N, +A] = ResponseT[EitherNec, R, N, A]

object ResponseE extends Response2Constructors

sealed trait Response2Constructors {
  def apply[R, E, A](
      result: EitherNec[R, A],
      notifications: Chain[E] = Chain.nil
  ): ResponseE[R, E, A] = ResponseT(result, notifications)

  /** constructs a program that outputs a pure value */
  def pure[R, N, T](t: T): ResponseE[R, N, T] = ResponseT(Right(t))

  /** a program with trivial output */
  def unit[R, N]: ResponseE[R, N, Unit] = pure(())

  /** constructs a program with given decision */
  def lift[R, N, T](d: EitherNec[R, T]): ResponseE[R, N, T] =
    ResponseT(d)

  /** constructs a program that publishes given notifications */
  def publish[R, N](n: N*): ResponseE[R, N, Unit] =
    ResponseT(Either.unit, Chain.fromSeq(n))

  /** constructs a program that rejects with given rejections */
  def reject[R, N](
      reason: R,
      otherReasons: R*
  ): ResponseE[R, N, Nothing] =
    ResponseT(Left(NonEmptyChain.of(reason, otherReasons: _*)))

  /** Constructs a program that uses a validation to decide whether to output a
    * value or reject with error(s)
    */
  def validate[R, N, T](
      validation: ValidatedNec[R, T]
  ): ResponseE[R, N, T] =
    ResponseT(validation.toEither)
}
