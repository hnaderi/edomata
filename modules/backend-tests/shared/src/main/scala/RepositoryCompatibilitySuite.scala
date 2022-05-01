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

import edomata.backend.AggregateState
import edomata.backend.CommandState
import edomata.backend.StreamId
import edomata.core.CommandMessage

import java.time.Instant

trait RepositoryCompatibilitySuite[S](
    existingCmdId: String,
    streamId: StreamId,
    state: AggregateState.Valid[S]
) {
  self: StorageSuite[S, ?, ?, ?] =>
  private val redundantCommand = CommandMessage(
    existingCmdId,
    Instant.MIN,
    streamId,
    ()
  )
  private val newCommand = redundantCommand.copy(id = s"new-$existingCmdId")

  check("Must load for non existing command id") { s =>
    s.repository.load(newCommand).assertEquals(state)
  }

  check("Must skip loading for existing command id") { s =>
    s.repository
      .load(redundantCommand)
      .assertEquals(CommandState.Redundant)
  }
}
