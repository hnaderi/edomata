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

trait Backend[F[_], S, E, R, N] {
  def compile: CommandHandler[F, S, E, R, N]
  def outbox: OutboxReader[F, N]
  def journal: JournalReader[F, E]
  def repository: RepositoryReader[F, S, E, R]
  def updates: NotificationsConsumer[F]
}

private[edomata] final case class BackendImpl[F[_], S, E, R, N](
    compile: CommandHandler[F, S, E, R, N],
    outbox: OutboxReader[F, N],
    journal: JournalReader[F, E],
    repository: RepositoryReader[F, S, E, R],
    updates: NotificationsConsumer[F]
) extends Backend[F, S, E, R, N]
