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

import cats.effect.Concurrent
import cats.effect.std.Queue
import cats.implicits.*
import fs2.Stream

trait NotificationsConsumer[F[_]] {
  def outbox: Stream[F, Unit]
  def journal: Stream[F, Unit]
}

trait NotificationsPublisher[F[_]] {
  def notifyOutbox: F[Unit]
  def notifyJournal: F[Unit]
}

trait Notifications[F[_]]
    extends NotificationsConsumer[F],
      NotificationsPublisher[F]

object Notifications {
  def apply[F[_]: Concurrent]: F[Notifications[F]] = for {
    o <- Queue.circularBuffer[F, Unit](1)
    j <- Queue.circularBuffer[F, Unit](1)
  } yield new {
    def outbox: Stream[F, Unit] = Stream.fromQueueUnterminated(o, 1)
    def journal: Stream[F, Unit] = Stream.fromQueueUnterminated(j, 1)
    def notifyOutbox: F[Unit] = o.offer(())
    def notifyJournal: F[Unit] = j.offer(())
  }
}
