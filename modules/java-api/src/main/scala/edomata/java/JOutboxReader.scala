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
import edomata.backend.OutboxItem
import edomata.backend.OutboxReader

import java.time.OffsetDateTime
import java.util.concurrent.CompletableFuture
import scala.jdk.CollectionConverters.*

/** Java-friendly wrapper for an outbox item. */
final class JOutboxItem[N](
    private[java] val underlying: OutboxItem[N]
) {
  def seqNr: Long = underlying.seqNr
  def streamId: String = underlying.streamId
  def time: OffsetDateTime = underlying.time
  def data: N = underlying.data
  override def toString: String =
    s"JOutboxItem(seqNr=$seqNr, streamId=$streamId, data=$data)"
}

/** Java-friendly outbox reader. FS2 streams are collected into lists. */
final class JOutboxReader[N] private[java] (
    private val underlying: OutboxReader[IO, N],
    private val runtime: EdomataRuntime
) {

  /** Read all pending outbox items. */
  def read(): CompletableFuture[java.util.List[JOutboxItem[N]]] =
    runtime.unsafeRunAsync(
      underlying.read
        .map(item => new JOutboxItem(item))
        .compile
        .toList
        .map(_.asJava)
    )
}
