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
import edomata.backend.eventsourcing.SnapshotPersistence
import edomata.backend.eventsourcing.AggregateState
import fs2.Chunk
import munit.Location

abstract class SnapshotPersistenceSuite(
    storage: Resource[IO, SnapshotPersistence[IO, Int]],
    name: String
) extends StorageSuite(storage, name) {

  check("Must read what's written single") { s =>
    for {
      id <- randomString
      state: AggregateState.Valid[Int] = AggregateState.Valid(1, 1)
      _ <- s.put(Chunk(id -> state))
      _ <- s.get(id).assertEquals(Some(state))
    } yield ()
  }

  check("Must read what's written chunk") { s =>
    for {
      id1 <- randomString
      state1: AggregateState.Valid[Int] = AggregateState.Valid(1, 1)

      id2 <- randomString
      state2: AggregateState.Valid[Int] = AggregateState.Valid(1, 1)
      _ <- s.put(Chunk(id1 -> state1, id2 -> state2))
      _ <- s.get(id1).assertEquals(Some(state1))
      _ <- s.get(id2).assertEquals(Some(state2))
    } yield ()
  }

  check("Must deduplicate write chunk") { s =>
    for {
      id <- randomString
      state: AggregateState.Valid[Int] = AggregateState.Valid(1, 1)
      _ <- s.put(Chunk(id -> state, id -> state))
      _ <- s.get(id).assertEquals(Some(state))
    } yield ()
  }

  check("Must write latest items in chunk") { s =>
    for {
      id <- randomString
      state1: AggregateState.Valid[Int] = AggregateState.Valid(1, 1)
      state2: AggregateState.Valid[Int] = AggregateState.Valid(3, 2)
      _ <- s.put(Chunk(id -> state1, id -> state2, id -> state1))
      _ <- s.get(id).assertEquals(Some(state2))
    } yield ()
  }

}
