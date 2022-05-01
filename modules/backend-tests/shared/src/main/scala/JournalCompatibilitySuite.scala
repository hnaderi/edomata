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

import edomata.backend.EventMessage

trait JournalCompatibilitySuite[E](events: List[EventMessage[E]]) {
  self: StorageSuite[?, E, ?, ?] =>
  private val streamId = ""
  private val journal = events.sortBy(_.metadata.seqNr)
  private val stream = journal.filter(_.metadata.stream == streamId)

  check("Must read all journal") { s =>
    s.journal.readAll.compile.toList.assertEquals(journal)
  }

  check("Must read all journal after") { s =>
    s.journal
      .readAllAfter(3)
      .compile
      .toList
      .assertEquals(
        journal.filter(_.metadata.seqNr > 3)
      )
  }

  check("Must read single stream from journal") { s =>
    s.journal
      .readStream(streamId)
      .compile
      .toList
      .assertEquals(stream)
  }

  check("Must read single stream from journal after") { s =>
    s.journal
      .readStreamAfter(streamId, 3)
      .compile
      .toList
      .assertEquals(
        stream.filter(_.metadata.seqNr > 3)
      )
  }

  check("Must read single stream from journal before") { s =>
    s.journal
      .readStreamBefore("", 3)
      .compile
      .toList
      .assertEquals(
        stream.filter(_.metadata.seqNr < 3)
      )
  }
}
