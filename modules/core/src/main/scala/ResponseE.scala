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
  * [[cats.data.EitherNec]] programs
  *
  * @tparam R
  *   rejection type
  * @tparam N
  *   notification type
  * @tparam A
  *   output type
  */
type ResponseE[+R, +N, +A] = ResponseT[EitherNec[R, *], R, N, A]

object ResponseE {
  def apply[R, N, A](
      result: EitherNec[R, A],
      notifications: Chain[N] = Chain.nil
  ): ResponseE[R, N, A] = ResponseT(result, notifications)

  /** constructs a program that outputs a pure value */
  def pure[R, N, T](t: T): ResponseE[R, N, T] = ResponseT(Right(t))

  /** a program with trivial output */
  def unit[R, N]: ResponseE[R, N, Unit] = pure(())

  /** constructs a program that publishes given notifications */
  def publish[R, N](n: N*): ResponseE[R, N, Unit] =
    ResponseT(Either.unit, Chain.fromSeq(n))

  /** constructs a program that rejects with given rejections */
  def reject[R, N, T](
      reason: R,
      otherReasons: R*
  ): ResponseE[R, N, T] =
    reject(NonEmptyChain.of(reason, otherReasons: _*))

  def reject[R, N, T](
      reasons: NonEmptyChain[R]
  ): ResponseE[R, N, T] =
    ResponseT(Left(reasons))

  /** Constructs a program that uses a validation to decide whether to output a
    * value or reject with error(s)
    */
  def validate[R, N, T](
      validation: ValidatedNec[R, T]
  ): ResponseE[R, N, T] =
    validation.fold(reject(_), pure(_))

  /** constructs a program with given decision */
  def validate[R, N, T](d: EitherNec[R, T]): ResponseE[R, N, T] =
    d.fold(reject(_), pure(_))
}
