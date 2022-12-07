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

import cats.effect.IO
import cats.effect.kernel.Ref
import edomata.core.*

object BlackHoleCommandStore extends CommandStore[IO] {
  def append(cmd: CommandMessage[?]): IO[Unit] = IO.unit
  def contains(id: String): IO[Boolean] = IO(false)
}

object YesManCommandStore extends CommandStore[IO] {
  def append(cmd: CommandMessage[?]): IO[Unit] = IO.unit
  def contains(id: String): IO[Boolean] = IO(true)
}

final class FakeCommandStore(
    states: Ref[IO, Set[String]]
) extends CommandStore[IO] {
  def append(cmd: CommandMessage[?]): IO[Unit] = states.update(_ + cmd.id)
  def contains(id: String): IO[Boolean] = states.get.map(_.contains(id))
  def all: IO[Set[String]] = states.get
}

object FakeCommandStore {
  def apply(): IO[FakeCommandStore] =
    IO.ref(Set.empty[String]).map(new FakeCommandStore(_))
}
