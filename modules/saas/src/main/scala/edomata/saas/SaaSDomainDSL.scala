/*
 * Copyright 2021 Beyond Scale Group
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

package edomata.saas

import cats.Applicative
import cats.Monad
import cats.data.*
import edomata.core.*

final class SaaSDomainDSL[Auth, C, A, E, R, N](
    mkRejection: String => R
)(using policy: AuthPolicy[Auth]):
  private val inner = DomainDSL[SaaSCommand[Auth, C], CrudState[A], E, R, N]()

  type App[F[_], T] =
    Edomaton[F, RequestContext[SaaSCommand[Auth, C], CrudState[A]], R, E, N, T]

  def auth[F[_]: Monad]: App[F, Auth] =
    inner.command.map(_.auth)

  def command[F[_]: Monad]: App[F, C] =
    inner.command.map(_.payload)

  def entityState[F[_]: Monad]: App[F, CrudState[A]] =
    inner.state

  private def liftEither[F[_]: Monad](
      result: Either[String, Unit]
  ): App[F, Unit] =
    result match
      case Right(()) => inner.unit
      case Left(msg) => inner.reject(mkRejection(msg))

  private def guard[F[_]: Monad](action: CrudAction): App[F, Unit] =
    for
      ctx <- inner.read
      a = ctx.command.payload.auth
      _ <- liftEither(SaaSGuard.checkTenant(ctx.state, a, action))
      _ <- liftEither(SaaSGuard.checkAuthorization(a, action))
    yield ()

  def guardedRouter[F[_]: Monad](
      f: C => (CrudAction, App[F, Unit])
  ): App[F, Unit] =
    command.flatMap { cmd =>
      val (action, logic) = f(cmd)
      guard(action) >> logic
    }

  def unsafeUnguardedRouter[F[_]: Monad](
      f: C => App[F, Unit]
  ): App[F, Unit] =
    command.flatMap(f)

  def guarded[F[_]: Monad](action: CrudAction)(
      logic: App[F, Unit]
  ): App[F, Unit] =
    guard(action) >> logic

  def unsafeUnguarded[F[_]: Monad](logic: App[F, Unit]): App[F, Unit] = logic

  def decide[F[_]: Applicative, T](d: Decision[R, E, T]): App[F, T] =
    inner.decide(d)

  def reject[F[_]: Applicative, T](r: R, rs: R*): App[F, T] =
    inner.reject(r, rs*)

  def publish[F[_]: Applicative](ns: N*): App[F, Unit] =
    inner.publish(ns*)

  def eval[F[_]: Applicative, T](f: F[T]): App[F, T] =
    inner.eval(f)

  def pure[F[_]: Monad, T](t: T): App[F, T] =
    inner.pure(t)

  def unit[F[_]: Monad]: App[F, Unit] =
    inner.unit

  def aggregateId[F[_]: Monad]: App[F, String] =
    inner.aggregateId

  def validate[F[_]: Applicative, T](v: ValidatedNec[R, T]): App[F, T] =
    inner.validate(v)
