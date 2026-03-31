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

package edomata.saas

import edomata.core.*

/** Base trait for event-sourced SaaS services.
  *
  * Extend this instead of `DomainModel#Service` to get automatic tenant
  * isolation and authorization guards. The raw `DomainDSL` is not accessible;
  * only the guarded `SaaS` DSL is exposed.
  *
  * @tparam C
  *   business command type (will be wrapped in `SaaSCommand[C]`)
  * @tparam A
  *   business entity state type (will be wrapped in `CrudState[A]`)
  * @tparam E
  *   domain event type
  * @tparam R
  *   rejection type
  * @tparam N
  *   notification type
  */
abstract class SaaSEventSourcedService[C, A, E, R, N](
    rolesFor: CrudAction => Set[String],
    mkRejection: String => R
)(using ModelTC[CrudState[A], E, R]):

  protected final val SaaS: SaaSDomainDSL[C, A, E, R, N] =
    SaaSDomainDSL(rolesFor, mkRejection)

  final type App[F[_], T] = SaaS.App[F, T]

  final type Handler[F[_]] =
    DomainService[F, CommandMessage[SaaSCommand[C]], R]

  final val domain: Domain[SaaSCommand[C], CrudState[A], E, R, N] = Domain()

/** Base trait for CQRS SaaS services.
  *
  * Extend this instead of `CQRSModel#Service` to get automatic tenant isolation
  * and authorization guards. The raw `CQRSDomainDSL` is not accessible; only
  * the guarded `SaaS` DSL is exposed.
  *
  * @tparam C
  *   business command type (will be wrapped in `SaaSCommand[C]`)
  * @tparam A
  *   business entity state type (will be wrapped in `CrudState[A]`)
  * @tparam R
  *   rejection type
  * @tparam N
  *   notification type
  */
abstract class SaaSCQRSService[C, A, R, N](
    rolesFor: CrudAction => Set[String],
    mkRejection: String => R
)(using StateModelTC[CrudState[A]]):

  protected final val SaaS: SaaSCQRSDomainDSL[C, A, R, N] =
    SaaSCQRSDomainDSL(rolesFor, mkRejection)

  final type App[F[_], T] = SaaS.App[F, T]

  final type Handler[F[_]] =
    DomainService[F, CommandMessage[SaaSCommand[C]], R]

  final val domain: CQRSDomain[SaaSCommand[C], CrudState[A], R, N] =
    CQRSDomain()
