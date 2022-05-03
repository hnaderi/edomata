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
import edomata.backend.Backend
import edomata.core.CommandMessage
import edomata.core.Edomaton
import edomata.core.Response
import munit.CatsEffectSuite
import tests.TestDomain.given_ModelTC_State_Event_Rejection

import java.time.Instant

abstract class PersistenceSuite(
    storage: Resource[IO, Backend[IO, Int, Int, String, Int]],
    name: String
) extends StorageSuite(storage, name) {

  private val someCmd = (randomString, randomString).mapN(
    CommandMessage(_, Instant.EPOCH, _, "command")
  )

  check("Must append correctly") { s =>
    for {
      cmd <- someCmd
      _ <- s
        .compile(Edomaton.lift(Response.accept(1, 2, 3).publish(4, 5, 6)))
        .apply(cmd)

      _ <- s.journal
        .readStream(cmd.address)
        .map(_.payload)
        .compile
        .toList
        .assertEquals(List(1, 2, 3))

      _ <- s.outbox.read
        .filter(_.streamId == cmd.address)
        .map(_.data)
        .compile
        .toList
        .assertEquals(List(4, 5, 6))
    } yield ()
  }

  check("Must notify correctly") { s =>
    for {
      cmd <- someCmd
      _ <- s
        .compile(Edomaton.lift(Response.publish(4, 5, 6)))
        .apply(cmd)

      _ <- s.journal
        .readStream(cmd.address)
        .compile
        .toList
        .assertEquals(Nil)

      _ <- s.outbox.read
        .filter(_.streamId == cmd.address)
        .map(_.data)
        .compile
        .toList
        .assertEquals(List(4, 5, 6))
    } yield ()
  }

  // check("Must read all journal") { s =>
  //   s.journal.readAll.compile.toList.assertEquals(journal)
  // }

  // check("Must read all journal after") { s =>
  //   s.journal
  //     .readAllAfter(3)
  //     .compile
  //     .toList
  //     .assertEquals(
  //       journal.filter(_.metadata.seqNr > 3)
  //     )
  // }

  // check("Must read single stream from journal") { s =>
  //   s.journal
  //     .readStream(streamId)
  //     .compile
  //     .toList
  //     .assertEquals(stream)
  // }

  // check("Must read single stream from journal after") { s =>
  //   s.journal
  //     .readStreamAfter(streamId, 3)
  //     .compile
  //     .toList
  //     .assertEquals(
  //       stream.filter(_.metadata.seqNr > 3)
  //     )
  // }

  // check("Must read single stream from journal before") { s =>
  //   s.journal
  //     .readStreamBefore("", 3)
  //     .compile
  //     .toList
  //     .assertEquals(
  //       stream.filter(_.metadata.seqNr < 3)
  //     )
  // }
}
