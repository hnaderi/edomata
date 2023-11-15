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
package eventsourcing

import cats.Monad
import cats.data.EitherNec
import cats.data.NonEmptyChain
import cats.effect.Temporal
import cats.implicits.*
import edomata.core.*

import scala.concurrent.duration.*
import cats.effect.std.Random

trait CommandHandler[F[_], S, E, R, N] {
  def apply[C](
      app: Edomaton[F, RequestContext[C, S], R, E, N, Unit]
  )(using ModelTC[S, E, R]): DomainService[F, CommandMessage[C], R]
}

object CommandHandler {
  private val void: EitherNec[Nothing, Unit] = Right(())

  private def run[F[_]: Monad, C, S, E, R, N](
      repository: Repository[F, S, E, R, N],
      app: Edomaton[F, RequestContext[C, S], R, E, N, Unit]
  )(using ModelTC[S, E, R]): DomainService[F, CommandMessage[C], R] =
    val voidF: F[EitherNec[R, Unit]] = void.pure[F]
    cmd =>
      repository.load(cmd).flatMap {
        case AggregateState.Valid(s, rev) =>
          val ctx = cmd.buildContext(s)
          val res = DomainCompiler.execute[F, C, S, E, R, N](app, ctx)

          res.flatMap {
            case EdomatonResult.Accepted(ns, evs, notifs) =>
              repository.append(ctx, rev, ns, evs, notifs).as(void)
            case EdomatonResult.Indecisive(notifs) =>
              NonEmptyChain
                .fromChain(notifs)
                .fold(voidF)(repository.notify(ctx, _).as(void))
            case EdomatonResult.Rejected(notifs, errs) =>
              val res = errs.asLeft[Unit]
              NonEmptyChain
                .fromChain(notifs)
                .fold(res.pure)(repository.notify(ctx, _).as(res))
            case EdomatonResult.Conflicted(errs) => errs.asLeft.pure
          }
        case AggregateState.Conflicted(ls, lEv, errs) => errs.asLeft.pure
        case CommandState.Redundant                   => voidF
      }

  def apply[F[_]: Monad, S, E, R, N](
      repository: Repository[F, S, E, R, N]
  ): CommandHandler[F, S, E, R, N] = new {
    def apply[C](
        app: Edomaton[F, RequestContext[C, S], R, E, N, Unit]
    )(using ModelTC[S, E, R]): DomainService[F, CommandMessage[C], R] =
      run(repository, app)
  }

  def withRetry[F[_]: Temporal: Random, S, E, R, N](
      repository: Repository[F, S, E, R, N],
      maxRetry: Int = 5,
      retryInitialDelay: FiniteDuration = 2.seconds
  ): CommandHandler[F, S, E, R, N] = new {
    def apply[C](
        app: Edomaton[F, RequestContext[C, S], R, E, N, Unit]
    )(using ModelTC[S, E, R]): DomainService[F, CommandMessage[C], R] =
      val handler = CommandHandler(repository).apply(app)
      cmd => retry(maxRetry, retryInitialDelay)(handler(cmd))
  }
}
