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
package kronikl

import cats.data.Chain
import cats.data.NonEmptyChain
import edomata.core.*

import java.time.OffsetDateTime
import java.util.UUID

final case class EventMetadata2(
    id: UUID,
    time: OffsetDateTime,
    seqNr: SeqNr,
    version: EventVersion,
    stream: String,
    entity: String,
    msgType: String
)

final case class JournalEntry[E](
    id: UUID,
    time: OffsetDateTime,
    version: EventVersion,
    stream: String,
    entity: String,
    msgType: String,
    payload: E
)

final case class JournalRecord[E](
    id: UUID,
    time: OffsetDateTime,
    version: EventVersion,
    seqNr: SeqNr,
    stream: String,
    entity: String,
    msgType: String,
    payload: E
)

final case class OutboxItem2[N](
    seqNr: SeqNr,
    streamId: StreamId,
    entity: String,
    time: OffsetDateTime,
    data: N,
    metadata: MessageMetadata,
    published: Boolean
)

trait JournalWriter[F[_], E] {
  def appendJournal(events: NonEmptyChain[JournalEntry[E]]): F[Unit]
}

trait OutboxWriter[F[_], N] {
  def insertNotification(notifications: NonEmptyChain[OutboxItem2[N]]): F[Unit]
  def markAsPublished(all: NonEmptyChain[SeqNr]): F[Unit]
  def cleanOldNotifications(maxToPreserve: Long): F[Unit]
}

trait CommandStorage[F[_]] {
  def rememberCommand(msg: CommandMessage[?]): F[Unit]
  def forgetOldCommands(maxToRemember: Long): F[Unit]
}

final case class JournalFilter(
    beforeTime: Option[OffsetDateTime] = None,
    afterTime: Option[OffsetDateTime] = None,
    beforeSeqNr: Option[SeqNr] = None,
    afterSeqNr: Option[SeqNr] = None,
    beforeVersion: Option[EventVersion] = None,
    afterVersion: Option[EventVersion] = None,
    streams: List[String] = Nil,
    entity: Option[String] = None,
    msgTypes: List[String] = Nil
)

final case class OutboxFilter(
    beforeTime: Option[OffsetDateTime] = None,
    afterTime: Option[OffsetDateTime] = None,
    beforeSeqNr: Option[SeqNr] = None,
    afterSeqNr: Option[SeqNr] = None,
    streams: List[String] = Nil,
    published: Option[Boolean] = Some(false)
)

trait StorageReader[F[_], E, N] {
  def readJournal(filter: JournalFilter): fs2.Stream[F, JournalRecord[E]]
  def readOutbox(filter: OutboxFilter): fs2.Stream[F, OutboxItem2[N]]
  def knowsCommand(cmd: CommandMessage[?]): F[Unit]
}
