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

import cats.data.*

trait RaiseError[F[_, _]] {
  def fold[E, O, A](v: F[E, O])(err: NonEmptyChain[E] => A, value: O => A): A
  def toEither[E, O](v: F[E, O]): EitherNec[E, O]
  def isError[E, O](v: F[E, O]): Boolean
}

object RaiseError {
  given RaiseError[EitherNec] = new {

    override def isError[E, O](v: EitherNec[E, O]): Boolean = v.isLeft

    override def toEither[E, O](v: EitherNec[E, O]): EitherNec[E, O] = v

    def fold[E, O, A](
        v: EitherNec[E, O]
    )(err: NonEmptyChain[E] => A, value: O => A): A = v.fold(err, value)
  }
  given RaiseError[ValidatedNec] = new {

    override def isError[E, O](v: ValidatedNec[E, O]): Boolean = v.isInvalid

    override def fold[E, O, A](
        v: ValidatedNec[E, O]
    )(err: NonEmptyChain[E] => A, value: O => A): A = v.fold(err, value)

    override def toEither[E, O](v: ValidatedNec[E, O]): EitherNec[E, O] =
      v.toEither

  }
  given [Ev]: RaiseError[Decision[*, Ev, *]] = new {

    override def isError[E, O](v: Decision[E, Ev, O]): Boolean = v.isRejected

    override def toEither[E, O](v: Decision[E, Ev, O]): EitherNec[E, O] =
      v.toEither

    def fold[E, O, A](
        v: Decision[E, Ev, O]
    )(err: NonEmptyChain[E] => A, value: O => A): A = v.visit(err, value)
  }
}
