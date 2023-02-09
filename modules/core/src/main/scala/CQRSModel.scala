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

final class CQRSDomain[C, S, R, N](
    private val dummy: Boolean = true
) extends AnyVal {
  def dsl: CQRSDomainDSL[C, S, R, N] = CQRSDomainDSL()
}

private[edomata] transparent trait CQRSDomainSyntax {
  extension [S, R](self: CQRSModel[S, R]) {
    def dsl[C, N]: CQRSDomainDSL[C, S, R, N] = CQRSDomainDSL()
    def domain[C, N]: CQRSDomain[C, S, R, N] = CQRSDomain()
  }
}

trait CQRSModel[State, Rejection] { self =>
  def initial: State

  given StateModelTC[State] = new {
    def initial: State = self.initial
  }

  trait Service[Command, Notification] {
    final val domain: CQRSDomain[Command, State, Rejection, Notification] =
      CQRSDomain()

    final protected val App
        : CQRSDomainDSL[Command, State, Rejection, Notification] = domain.dsl

    final type Handler[F[_]] =
      DomainService[F, CommandMessage[Command], Rejection]

    final type App[F[_], T] =
      Stomaton[F, CommandMessage[Command], State, Rejection, Notification, T]
    final type PureApp[T] = App[cats.Id, T]
    val dsl = CQRSDomainDSL[Command, State, Rejection, Notification]()
  }
}
