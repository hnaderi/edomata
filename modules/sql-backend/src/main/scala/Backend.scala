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

import cats.data.EitherNec
import cats.effect.Temporal
import cats.effect.implicits.*
import cats.implicits.*
import edomata.core.*

import scala.concurrent.duration.*

trait Backend[F[_], S, E, R, N] {
  def compile[C](
      app: Edomaton[F, RequestContext[C, S], R, E, N, Unit]
  ): DomainService[F, CommandMessage[C], R]
  val outbox: OutboxReader[F, N]
  val journal: JournalReader[F, E]
  val repository: RepositoryReader[F, S, E, R]
}

object Backend {
  def apply[F[_]: Temporal, S, E, R, N](
      compiler: Repository[F, S, E, R, N],
      _outbox: OutboxReader[F, N],
      _journal: JournalReader[F, E],
      _repo: RepositoryReader[F, S, E, R],
      maxRetry: Int = 5,
      retryInitialDelay: FiniteDuration = 2.seconds
  )(using ModelTC[S, E, R]): Backend[F, S, E, R, N] = new {
    def compile[C](
        app: Edomaton[F, RequestContext[C, S], R, E, N, Unit]
    ): DomainService[F, CommandMessage[C], R] = cmd =>
      retry(maxRetry, retryInitialDelay) {
        CommandHandler(compiler, app).apply(cmd)
      }
    val outbox: OutboxReader[F, N] = _outbox
    val journal: JournalReader[F, E] = _journal
    val repository: RepositoryReader[F, S, E, R] = _repo
  }

  private def retry[F[_]: Temporal, T](max: Int, wait: FiniteDuration)(
      f: F[T]
  ): F[T] =
    f.recoverWith {
      case BackendError.VersionConflict if max > 0 =>
        retry(max - 1, wait * 2)(f).delayBy(wait)
    }.adaptErr { case BackendError.VersionConflict =>
      BackendError.MaxRetryExceeded
    }
}
