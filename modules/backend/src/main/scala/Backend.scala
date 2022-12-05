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

import cats.effect.kernel.Async
import cats.effect.kernel.Resource
import edomata.core.*

import scala.concurrent.duration.*

object Backend {
  def builder[C, S, E, R, N](
      domain: Domain[C, S, E, R, N]
  )(using
      model: ModelTC[S, E, R]
  ): eventsourcing.PartialBackendBuilder[C, S, E, R, N] =
    new eventsourcing.PartialBackendBuilder(domain)
  def builder[C, S, E, R, N](
      service: edomata.core.DomainModel[S, E, R]#Service[C, N]
  )(using
      model: ModelTC[S, E, R]
  ): eventsourcing.PartialBackendBuilder[C, S, E, R, N] =
    new eventsourcing.PartialBackendBuilder(service.domain)

  def builder[C, S, R, N](
      domain: CQRSDomain[C, S, R, N]
  )(using
      model: StateModelTC[S]
  ): cqrs.PartialBackendBuilder[C, S, R, N] =
    new cqrs.PartialBackendBuilder(domain)

  def builder[C, S, R, N](
      service: CQRSModel[S, R]#Service[C, N]
  )(using StateModelTC[S]): cqrs.PartialBackendBuilder[C, S, R, N] =
    new cqrs.PartialBackendBuilder(service.domain)
}
