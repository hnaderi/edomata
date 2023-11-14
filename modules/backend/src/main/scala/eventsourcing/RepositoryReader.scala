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
package eventsourcing

import cats.data.NonEmptyChain
import cats.effect.Concurrent
import cats.implicits.*
import edomata.core.*
import fs2.Pipe
import fs2.Stream

type CommandState[S, E, R] = AggregateState[S, E, R] |
  CommandState.Redundant.type

enum AggregateState[+S, +E, +R](val isValid: Boolean) {
  case Valid[S](state: S, version: SeqNr)
      extends AggregateState[S, Nothing, Nothing](true)
  case Conflicted[S, E, R](
      last: S,
      onEvent: EventMessage[E],
      errors: NonEmptyChain[R]
  ) extends AggregateState[S, E, R](false)
}

trait RepositoryReader[F[_], S, E, R] {
  def get(streamId: StreamId): F[AggregateState[S, E, R]]
  def history(streamId: StreamId): Stream[F, AggregateState[S, E, R]]
}

object RepositoryReader {
  def apply[F[_], S, E, R](
      journal: JournalReader[F, E],
      snapshot: SnapshotReader[F, S]
  )(using F: Concurrent[F], m: ModelTC[S, E, R]): RepositoryReader[F, S, E, R] =
    new {
      def get(streamId: StreamId): F[AggregateState[S, E, R]] =
        snapshot
          .get(streamId)
          .flatMap {
            case Some(last) =>
              journal
                .readStreamAfter(streamId, last.version)
                .through(scanState(last))
                .compile
                .lastOrError
            case None => history(streamId).compile.lastOrError
          }

      def history(streamId: StreamId): Stream[F, AggregateState[S, E, R]] =
        journal
          .readStream(streamId)
          .through(scanState(AggregateState.Valid(m.initial, 0L)))
    }

  private def scanState[F[_], S, E, R](
      last: AggregateState[S, E, R]
  )(using
      m: ModelTC[S, E, R]
  ): Pipe[F, EventMessage[E], AggregateState[S, E, R]] =
    _.scan(last) {
      case (AggregateState.Valid(s, version), ev) =>
        m.transition(ev.payload)(s)
          .fold(
            AggregateState.Conflicted(s, ev, _),
            AggregateState.Valid(_, version + 1)
          )
      case (other, ev) => other
    }.takeWhile(_.isValid, true)
}
