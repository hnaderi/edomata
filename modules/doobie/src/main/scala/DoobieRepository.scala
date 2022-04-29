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

package edomata.backend
package doobie

import _root_.doobie.ConnectionIO
import _root_.doobie.FC
import _root_.doobie.Transactor
import _root_.doobie.implicits.*
import cats.data.Chain
import cats.data.EitherNec
import cats.data.NonEmptyChain
import cats.effect.Concurrent
import cats.effect.Temporal
import cats.effect.kernel.Clock
import cats.effect.kernel.Resource
import cats.implicits.*
import edomata.core.*
import fs2.Stream

import java.time.OffsetDateTime
import java.time.ZoneOffset

private final class DoobieRepository[F[_], S, E, R, N](trx: Transactor[F])(using
    F: Concurrent[F],
    clock: Clock[F]
) extends Repository[F, S, E, R, N] {
  def load(cmd: CommandMessage[?]): F[CommandState[S, E, R]] = ???
  def append(
      ctx: RequestContext[?, ?],
      version: SeqNr,
      newState: S,
      events: NonEmptyChain[E],
      notifications: Chain[N]
  ): F[Unit] = ???

  def notify(
      ctx: RequestContext[?, ?],
      notifications: NonEmptyChain[N]
  ): F[Unit] = ???
}
