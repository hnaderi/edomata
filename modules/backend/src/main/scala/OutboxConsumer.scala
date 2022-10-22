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

import cats.Monad
import cats.data.NonEmptyChain
import cats.implicits.*
import edomata.backend.Backend
import edomata.backend.OutboxItem
import fs2.Stream
import fs2.Stream.*

object OutboxConsumer {
  def apply[F[_], S, E, R, N](
      backend: Backend[F, S, E, R, N]
  )(
      run: edomata.backend.OutboxItem[N] => F[Unit]
  )(using F: Monad[F]): Stream[F, Nothing] =
    (emit(()) ++ backend.updates.outbox)
      .flatMap(_ => backend.outbox.read)
      .chunks
      .foreach(ch =>
        NonEmptyChain
          .fromChain(ch.toChain)
          .fold(F.unit)(
            ch.traverse(run) >> backend.outbox.markAllAsSent(_)
          )
      )
}
