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
package cqrs

import cats.Monad
import cats.data.*
import cats.effect.Temporal
import cats.effect.implicits.*
import cats.implicits.*
import edomata.core.*

import scala.concurrent.duration.*

trait CommandHandler[F[_], S, E] {
  def apply[C, R](
      app: Stomaton[F, CommandMessage[C], S, R, E, Unit]
  )(using StateModelTC[S]): DomainService[F, CommandMessage[C], R]
}

object CommandHandler {
  private val void: EitherNec[Nothing, Unit] = Right(())

  private def run[F[_]: Monad, C, S, E, R](
      repository: Repository[F, S, E],
      app: Stomaton[F, CommandMessage[C], S, R, E, Unit]
  )(using StateModelTC[S]): DomainService[F, CommandMessage[C], R] =
    val voidF: F[EitherNec[R, Unit]] = void.pure[F]
    cmd =>
      repository.load(cmd).flatMap {
        case AggregateS(state, version) =>
          app.run(cmd, state).flatMap { out =>
            out.result match {
              case Right((newState, _)) =>
                repository
                  .save(cmd, version, newState, out.notifications) >> voidF
              case Left(reasons) =>
                reasons.asLeft.pure
            }

          }
        case CommandState.Redundant => voidF
      }

  def apply[F[_]: Monad, S, E, R](
      repository: Repository[F, S, E]
  ): CommandHandler[F, S, E] = new {

    override def apply[C, R](
        app: Stomaton[F, CommandMessage[C], S, R, E, Unit]
    )(using StateModelTC[S]): DomainService[F, CommandMessage[C], R] =
      run(repository, app)

  }

  def withRetry[F[_]: Temporal, S, E, R](
      repository: Repository[F, S, E],
      maxRetry: Int = 5,
      retryInitialDelay: FiniteDuration = 2.seconds
  ): CommandHandler[F, S, E] = new {

    override def apply[C, R](
        app: Stomaton[F, CommandMessage[C], S, R, E, Unit]
    )(using StateModelTC[S]): DomainService[F, CommandMessage[C], R] = ???

  }

  private def retry[F[_]: Temporal, T](max: Int, wait: FiniteDuration)(
      f: F[T]
  ): F[T] =
    f.recoverWith {
      case BackendError.VersionConflict if max > 1 =>
        retry(max - 1, wait * 2)(f).delayBy(wait)
    }.adaptErr { case BackendError.VersionConflict =>
      BackendError.MaxRetryExceeded
    }
}

import cats.data.*
trait Repository[F[_], S, E] {
  def load(cmd: CommandMessage[?]): F[AggregateState[S]]

  def save(
      ctx: CommandMessage[?],
      version: SeqNr,
      newState: S,
      events: Chain[E]
  ): F[Unit]
}

final case class AggregateS[S](state: S, version: SeqNr)
type AggregateState[S] = AggregateS[S] | CommandState.Redundant.type
