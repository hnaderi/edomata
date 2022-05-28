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
import cats.Monad
import cats.data.*
import cats.implicits.*

final class Domain[C, S, E, R, N](
    private val dummy: Boolean = true
) extends AnyVal {
  def dsl: DomainDSL[C, S, E, R, N] = DomainDSL()
}

final class DomainDSL[C, S, E, R, N](
    private val dummy: Boolean = true
) extends AnyVal {
  type App[F[_], T] = Edomaton[F, RequestContext[C, S], R, E, N, T]

  inline def pure[F[_]: Monad, T](
      t: T
  ): App[F, T] =
    Edomaton.pure(t)

  inline def unit[F[_]: Monad]: App[F, Unit] =
    Edomaton.unit

  inline def liftF[F[_], T](
      f: F[Response[R, E, N, T]]
  ): App[F, T] = Edomaton.liftF(f)

  inline def lift[F[_]: Applicative, T](
      f: Response[R, E, N, T]
  ): App[F, T] = Edomaton.lift(f)

  inline def eval[F[_]: Applicative, T](
      f: F[T]
  ): App[F, T] = Edomaton.eval(f)

  inline def run[F[_]: Applicative, T](
      f: RequestContext[C, S] => F[T]
  ): App[F, T] = Edomaton.run(f)

  inline def read[F[_]: Applicative]: App[F, RequestContext[C, S]] =
    Edomaton.read

  inline def publish[F[_]: Applicative](
      ns: N*
  ): App[F, Unit] =
    Edomaton.publish(ns: _*)

  inline def reject[F[_]: Applicative, T](
      r: R,
      rs: R*
  ): App[F, T] =
    Edomaton.reject(r, rs: _*)

  inline def decide[F[_]: Applicative, T](
      d: Decision[R, E, T]
  ): App[F, T] =
    Edomaton.decide(d)

  inline def validate[F[_]: Applicative, T](
      v: ValidatedNec[R, T]
  ): App[F, T] =
    Edomaton.validate(v)

  inline def fromOption[F[_]: Applicative, Env, R, E, N, T](
      opt: Option[T],
      orElse: R,
      other: R*
  ): Edomaton[F, Env, R, E, N, T] = Edomaton.fromOption(opt, orElse, other: _*)

  inline def fromEither[F[_]: Applicative, Env, R, E, N, T](
      eit: Either[R, T]
  ): Edomaton[F, Env, R, E, N, T] = Edomaton.fromEither(eit)

  inline def fromEitherNec[F[_]: Applicative, Env, R, E, N, T](
      eit: EitherNec[R, T]
  ): Edomaton[F, Env, R, E, N, T] = Edomaton.fromEitherNec(eit)

  def state[F[_]: Monad]: App[F, S] =
    Edomaton.read.map(_.state)

  def aggregateId[F[_]: Monad]: App[F, String] =
    Edomaton.read.map(_.command.address)

  def metadata[F[_]: Monad]: App[F, MessageMetadata] =
    Edomaton.read.map(_.command.metadata)

  def messageId[F[_]: Monad]: App[F, String] =
    Edomaton.read.map(_.command.id)

  def command[F[_]: Monad]: App[F, C] =
    Edomaton.read.map(_.command.payload)

  def router[F[_]: Monad, T](
      f: C => App[F, T]
  ): App[F, T] =
    command.flatMap(f)

}

private[edomata] transparent trait DomainSyntax {
  extension [S, E, R](self: DomainModel[S, E, R]) {
    def dsl[C, N]: DomainDSL[C, S, E, R, N] = DomainDSL()
    def domain[C, N]: Domain[C, S, E, R, N] = Domain()
  }
}
