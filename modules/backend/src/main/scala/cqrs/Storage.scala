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

import cats.effect.kernel.Resource
import edomata.core.StateModelTC

final case class Storage[F[_], S, N, R](
    repository: Repository[F, S, N],
    outbox: OutboxReader[F, N],
    updates: NotificationsConsumer[F]
)

trait StorageDriver[F[_], Codec[_]] {
  def build[S, N, R](using
      StateModelTC[S],
      Codec[S],
      Codec[N]
  ): Resource[F, Storage[F, S, N, R]]
}
