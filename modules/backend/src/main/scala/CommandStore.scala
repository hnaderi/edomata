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
import cats.effect.kernel.Async
import cats.effect.kernel.Ref
import cats.implicits.*
import edomata.core.*

import scala.collection.immutable.HashSet

trait CommandStore[F[_]] {
  def append(cmd: CommandMessage[?]): F[Unit]
  def contains(id: String): F[Boolean]
}

object CommandStore {
  def inMem[F[_]](size: Int)(using F: Async[F]): F[CommandStore[F]] =
    (LRUCache[F, String, Unit](size), F.ref(HashSet.empty[String]))
      .mapN(InMemoryCommandStore(_, _))

  private[backend] final class InMemoryCommandStore[F[_]: Monad](
      cache: Cache[F, String, Unit],
      set: Ref[F, HashSet[String]]
  ) extends CommandStore[F] {
    def append(cmd: CommandMessage[?]): F[Unit] =
      cache.add(cmd.id, ()).flatMap {
        case Some((id, _)) => set.update(_ - id + cmd.id)
        case None          => set.update(_ + cmd.id)
      }
    def contains(id: String): F[Boolean] = set.get.map(_.contains(id))
  }
}
