/*
 * Copyright 2021 Beyond Scale Group
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

package edomata.java

import cats.effect.IO
import edomata.backend.eventsourcing.JournalReader

import java.util.concurrent.CompletableFuture
import scala.jdk.CollectionConverters.*

/** Java-friendly journal reader. FS2 streams are collected into lists. */
final class JJournalReader[E] private[java] (
    private val underlying: JournalReader[IO, E],
    private val runtime: EdomataRuntime
) {

  /** Read all events for a given stream (aggregate). */
  def readStream(
      streamId: String
  ): CompletableFuture[java.util.List[JEventMessage[E]]] =
    runtime.unsafeRunAsync(
      underlying
        .readStream(streamId)
        .map(JEventMessage.fromScala)
        .compile
        .toList
        .map(_.asJava)
    )

  /** Read all events for a given stream after a specific version. */
  def readStreamAfter(
      streamId: String,
      version: Long
  ): CompletableFuture[java.util.List[JEventMessage[E]]] =
    runtime.unsafeRunAsync(
      underlying
        .readStreamAfter(streamId, version)
        .map(JEventMessage.fromScala)
        .compile
        .toList
        .map(_.asJava)
    )

  /** Read all events across all streams. */
  def readAll(): CompletableFuture[java.util.List[JEventMessage[E]]] =
    runtime.unsafeRunAsync(
      underlying.readAll
        .map(JEventMessage.fromScala)
        .compile
        .toList
        .map(_.asJava)
    )

  /** Read all events across all streams after a specific sequence number. */
  def readAllAfter(
      seqNr: Long
  ): CompletableFuture[java.util.List[JEventMessage[E]]] =
    runtime.unsafeRunAsync(
      underlying
        .readAllAfter(seqNr)
        .map(JEventMessage.fromScala)
        .compile
        .toList
        .map(_.asJava)
    )
}
