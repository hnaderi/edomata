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

import cats.data.NonEmptyChainImpl.Type
import cats.data.*

trait RaiseError[F[+_], R] {
  def fold[O, A](v: F[O])(err: NonEmptyChain[R] => A, value: O => A): A
  def toEither[O](v: F[O]): EitherNec[R, O]
  def isError[O](v: F[O]): Boolean
  def raise(errs: NonEmptyChain[R]): F[Nothing]
  def pure[T](t: T): F[T]
  val unit = pure(())
}

object RaiseError {
  given [R]: RaiseError[EitherNec[R, *], R] = new {

    override def raise(errs: Type[R]): EitherNec[R, Nothing] = Left(errs)

    override def pure[T](t: T): EitherNec[R, T] = Right(t)

    override def isError[O](v: EitherNec[R, O]): Boolean = v.isLeft

    override def toEither[O](v: EitherNec[R, O]): EitherNec[R, O] = v

    override def fold[O, A](
        v: EitherNec[R, O]
    )(err: Type[R] => A, value: O => A): A = v.fold(err, value)

  }
  given [R, E]: RaiseError[Decision[R, E, *], R] = new {

    override def raise(errs: Type[R]): Decision[R, E, Nothing] =
      Decision.Rejected(errs)

    override def pure[T](t: T): Decision[R, E, T] = Decision(t)

    override def isError[O](v: Decision[R, E, O]): Boolean = v.isRejected

    override def toEither[O](v: Decision[R, E, O]): EitherNec[R, O] = v.toEither

    override def fold[O, A](
        v: Decision[R, E, O]
    )(err: Type[R] => A, value: O => A): A = v.visit(err, value)
  }
  given [R]: RaiseError[ValidatedNec[R, *], R] = new {

    override def raise(errs: Type[R]): ValidatedNec[R, Nothing] =
      Validated.Invalid(errs)

    override def pure[T](t: T): ValidatedNec[R, T] = Validated.Valid(t)

    override def isError[O](v: ValidatedNec[R, O]): Boolean = v.isInvalid

    override def toEither[O](v: ValidatedNec[R, O]): EitherNec[R, O] =
      v.toEither

    override def fold[O, A](
        v: ValidatedNec[R, O]
    )(err: Type[R] => A, value: O => A): A = v.fold(err, value)

  }
}
