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
import fs2.Stream

import FakeOutboxReader.*
import cats.data.NonEmptyChain

final class FakeOutboxReader[N](
    actions: Ref[IO, List[Marked[N]]],
    data: Stream[IO, OutboxItem[N]]
) extends OutboxReader[IO, N] {
  override def read: Stream[IO, OutboxItem[N]] = data
  override def markAllAsSent(items: NonEmptyChain[OutboxItem[N]]): IO[Unit] =
    actions.update(_.prepended(Marked(items)))
  def listActions: IO[List[Marked[N]]] = actions.get
}
object FakeOutboxReader {
  final case class Marked[N](items: NonEmptyChain[OutboxItem[N]])

  def apply[N](data: Stream[IO, OutboxItem[N]]): IO[FakeOutboxReader[N]] =
    IO.ref(List.empty[Marked[N]]).map(new FakeOutboxReader(_, data))

}
