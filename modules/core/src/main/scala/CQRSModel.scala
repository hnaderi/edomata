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

package edomata.core

trait StateModelTC[State] {
  def initial: State
}

final class CQRSDomain[C, S, E, R](
    private val dummy: Boolean = true
) extends AnyVal {
  def dsl: CQRSDomainDSL[C, S, E, R] = CQRSDomainDSL()
}

private[edomata] transparent trait CQRSDomainSyntax {
  extension [S, E, R](self: CQRSModel[S, E, R]) {
    def dsl[C]: CQRSDomainDSL[C, S, E, R] = CQRSDomainDSL()
    def domain[C]: CQRSDomain[C, S, E, R] = CQRSDomain()
  }
}

trait CQRSModel[State, Event, Rejection] { self =>
  def initial: State

  given StateModelTC[State] = new {
    def initial: State = self.initial
  }

  trait Service[Command] {
    final type App[F[_], T] =
      Stomaton[F, CommandMessage[Command], State, Rejection, Event, T]
    final type PureApp[T] = App[cats.Id, T]
    val dsl = CQRSDomainDSL[Command, State, Event, Rejection]()
  }
}
