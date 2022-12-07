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

@deprecated("use edomata.backend.eventsourcing instead")
type CommandHandler[F[_], S, E, R, N] =
  eventsourcing.CommandHandler[F, S, E, R, N]
@deprecated("use edomata.backend.eventsourcing instead")
val CommandHandler = eventsourcing.CommandHandler

@deprecated("use edomata.backend.eventsourcing instead")
type JournalReader[F[_], E] = eventsourcing.JournalReader[F, E]

@deprecated("use edomata.backend.eventsourcing instead")
type Repository[F[_], S, E, R, N] = eventsourcing.Repository[F, S, E, R, N]

@deprecated("use edomata.backend.eventsourcing instead")
type RepositoryReader[F[_], S, E, R] =
  eventsourcing.RepositoryReader[F, S, E, R]

@deprecated("use edomata.backend.eventsourcing instead")
val RepositoryReader = eventsourcing.RepositoryReader

@deprecated("use edomata.backend.eventsourcing instead")
type Storage[F[_], S, E, R, N] = eventsourcing.Storage[F, S, E, R, N]

@deprecated("use edomata.backend.eventsourcing instead")
type StorageDriver[F[_], Codec[_]] = eventsourcing.StorageDriver[F, Codec]

@deprecated("use edomata.backend.eventsourcing instead")
type AggregateState[S, E, R] = eventsourcing.AggregateState[S, E, R]

@deprecated("use edomata.backend.eventsourcing instead")
val AggregateState = eventsourcing.AggregateState

@deprecated("use edomata.backend.eventsourcing instead")
type CommandState[S, E, R] = eventsourcing.CommandState[S, E, R]

@deprecated("use edomata.backend.eventsourcing instead")
type NotificationsConsumer[F[_]] = eventsourcing.NotificationsConsumer[F]

@deprecated("use edomata.backend.eventsourcing instead")
type NotificationsPublisher[F[_]] = eventsourcing.NotificationsPublisher[F]

@deprecated("use edomata.backend.eventsourcing instead")
type Notifications[F[_]] = eventsourcing.Notifications[F]

@deprecated("use edomata.backend.eventsourcing instead")
type SnapshotReader[F[_], S] = eventsourcing.SnapshotReader[F, S]

@deprecated("use edomata.backend.eventsourcing instead")
type SnapshotStore[F[_], S] = eventsourcing.SnapshotStore[F, S]

@deprecated("use edomata.backend.eventsourcing instead")
type SnapshotPersistence[F[_], S] = eventsourcing.SnapshotPersistence[F, S]

@deprecated("use edomata.backend.eventsourcing instead")
val SnapshotStore = eventsourcing.SnapshotStore

@deprecated("use edomata.backend.eventsourcing instead")
type SnapshotItem[S] = eventsourcing.SnapshotItem[S]
