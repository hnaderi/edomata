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

import cats.effect.kernel.Resource
import edomata.core.ModelTC

trait StorageDriver[F[_], Codec[_]] {
  def build[S, E, R, N](snapshot: SnapshotStore[F, S])(using
      ModelTC[S, E, R],
      Codec[E],
      Codec[N]
  ): Resource[F, Storage[F, S, E, R, N]]
  def snapshot[S](using Codec[S]): Resource[F, SnapshotPersistence[F, S]]
}
