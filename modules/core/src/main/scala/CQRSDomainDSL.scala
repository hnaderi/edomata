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

import cats.implicits.*
import cats.*
import cats.data.*

final class CQRSDomainDSL[C, S, E, R](
    private val dummy: Boolean = true
) extends AnyVal {
  type App[F[_], T] = Stomaton[F, CommandMessage[C], S, R, E, T]

  inline def pure[F[_]: Monad, T](
      t: T
  ): App[F, T] =
    Stomaton.pure(t)

  inline def unit[F[_]: Monad]: App[F, Unit] =
    Stomaton.unit

  inline def eval[F[_]: Applicative, T](
      f: F[T]
  ): App[F, T] = Stomaton.eval(f)

  /** constructs an stomaton that outputs what's read */
  inline def modify[F[_]: Applicative](
      f: S => S
  ): App[F, S] =
    Stomaton.modify(f)

  /** constructs an stomaton that outputs what's read */
  inline def modifyS[F[_]: Applicative](
      f: S => Decision[R, E, S]
  ): App[F, S] =
    Stomaton.modifyS(f)

  inline def reject[F[_]: Applicative, T](
      r: R,
      rs: R*
  ): App[F, T] =
    Stomaton.reject(r, rs: _*)

  inline def decide[F[_]: Applicative, T](
      d: Decision[R, E, T]
  ): App[F, T] =
    Stomaton.decide(d)

  inline def validate[F[_]: Applicative, T](
      v: ValidatedNec[R, T]
  ): App[F, T] =
    Stomaton.validate(v)

  inline def fromOption[F[_]: Applicative, T](
      opt: Option[T],
      orElse: R,
      other: R*
  ): App[F, T] = Stomaton.fromOption(opt, orElse, other: _*)

  inline def fromEither[F[_]: Applicative, T](
      eit: Either[R, T]
  ): App[F, T] = Stomaton.fromEither(eit)

  inline def fromEitherNec[F[_]: Applicative, T](
      eit: EitherNec[R, T]
  ): App[F, T] = Stomaton.fromEitherNec(eit)

  def state[F[_]: Monad]: App[F, S] =
    Stomaton.state

  def aggregateId[F[_]: Monad]: App[F, String] =
    Stomaton.context.map(_.address)

  def metadata[F[_]: Monad]: App[F, MessageMetadata] =
    Stomaton.context.map(_.metadata)

  def messageId[F[_]: Monad]: App[F, String] =
    Stomaton.context.map(_.id)

  def command[F[_]: Monad]: App[F, C] =
    Stomaton.context.map(_.payload)

  def router[F[_]: Monad, T](
      f: C => App[F, T]
  ): App[F, T] =
    command.flatMap(f)

}
