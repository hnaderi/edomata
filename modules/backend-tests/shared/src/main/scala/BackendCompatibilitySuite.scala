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

package tests

import cats.effect.IO
import cats.effect.kernel.Resource
import cats.implicits.*
import edomata.backend.EventMessage
import edomata.backend.EventMetadata
import edomata.backend.OutboxItem
import edomata.backend.StreamId
import edomata.backend.eventsourcing.*
import edomata.core.CommandMessage
import edomata.core.Edomaton
import edomata.core.MessageMetadata
import edomata.core.RequestContext

import java.time.Instant
import java.time.ZoneOffset
import java.util.UUID

import TestDomain.given_ModelTC_State_Event_Rejection

abstract class BackendCompatibilitySuite(
    storage: Resource[IO, Backend[IO, Int, Int, String, Int]],
    name: String
) extends StorageSuite(storage, s"$name compatibility") {

  check("Must read all journal") { s =>
    s.journal.readAll.compile.toList.assertEquals(PreparedData.journal)
  }

  check("Must read all outbox items") { s =>
    s.outbox.read
      .take(PreparedData.outbox.size)
      .compile
      .toList
      .assertEquals(PreparedData.outbox.sortBy(_.seqNr))
  }

  private val redundantCommand = CommandMessage(
    PreparedData.redundantCmd,
    Instant.MIN,
    PreparedData.streamId,
    ()
  )

  check("Must load for non existing command id") { s =>
    val newCommand = redundantCommand.copy(id = s"new-${redundantCommand.id}")
    for {
      ref <- IO.ref(Option.empty[RequestContext[Any, Int]])
      _ <- s.compile(Edomaton.run(e => ref.set(e.some))).apply(newCommand)
      _ <- ref.get.assertEquals(
        Some(
          RequestContext(newCommand, 12)
        )
      )
    } yield ()
  }

  check("Must skip loading for existing command id") { s =>
    for {
      ref <- IO.ref(0)
      _ <- s.compile(Edomaton.eval(ref.update(_ + 1))).apply(redundantCommand)
      _ <- ref.get.assertEquals(0)
    } yield ()
  }
}

object PreparedData {
  private val epoch = Instant.EPOCH.atOffset(ZoneOffset.UTC)

  val journal: List[EventMessage[Int]] = List(
    EventMessage(
      EventMetadata(
        UUID(0, 0),
        epoch,
        seqNr = 1,
        version = 0,
        "a"
      ),
      1234
    )
  )
  val outbox: List[OutboxItem[Int]] = List(
    OutboxItem(
      seqNr = 1,
      streamId = "a",
      epoch,
      123456,
      MessageMetadata("correlation", "causation")
    )
  )

  val streamId: StreamId = "a"
  val aggregate: AggregateState.Valid[Int] = AggregateState.Valid(12, 1)
  val redundantCmd: String = "redundant"
}
