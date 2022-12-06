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

@deprecated("use edomata.backend.eventsourcing instead")
type Backend[F[_], S, E, R, N] = eventsourcing.Backend[F, S, E, R, N]

type CommandHandler[F[_], S, E, R, N] =
  eventsourcing.CommandHandler[F, S, E, R, N]
val CommandHandler = eventsourcing.CommandHandler

type JournalReader[F[_], E] = eventsourcing.JournalReader[F, E]
type Repository[F[_], S, E, R, N] = eventsourcing.Repository[F, S, E, R, N]
type RepositoryReader[F[_], S, E, R] =
  eventsourcing.RepositoryReader[F, S, E, R]
val RepositoryReader = eventsourcing.RepositoryReader
type Storage[F[_], S, E, R, N] = eventsourcing.Storage[F, S, E, R, N]
type StorageDriver[F[_], Codec[_]] = eventsourcing.StorageDriver[F, Codec]
type AggregateState[S, E, R] = eventsourcing.AggregateState[S, E, R]
val AggregateState = eventsourcing.AggregateState
type CommandState[S, E, R] = eventsourcing.CommandState[S, E, R]

type NotificationsConsumer[F[_]] = eventsourcing.NotificationsConsumer[F]
type NotificationsPublisher[F[_]] = eventsourcing.NotificationsPublisher[F]
type Notifications[F[_]] = eventsourcing.Notifications[F]
