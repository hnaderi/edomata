/*
 * Copyright 2021 Beyond Scale Group
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
  * @tparam Auth
  *   authentication/authorization context type
  * @tparam C
  *   business command type (will be wrapped in `SaaSCommand[Auth, C]`)
  * @tparam A
  *   business entity state type (will be wrapped in `CrudState[A]`)
  * @tparam E
  *   domain event type
  * @tparam R
  *   rejection type
  * @tparam N
  *   notification type
  */
abstract class SaaSEventSourcedService[Auth, C, A, E, R, N](
    mkRejection: String => R
)(using ModelTC[CrudState[A], E, R], AuthPolicy[Auth]):

  protected final val SaaS: SaaSDomainDSL[Auth, C, A, E, R, N] =
    SaaSDomainDSL(mkRejection)

  final type App[F[_], T] = SaaS.App[F, T]

  final type Handler[F[_]] =
    DomainService[F, CommandMessage[SaaSCommand[Auth, C]], R]

  final val domain: Domain[SaaSCommand[Auth, C], CrudState[A], E, R, N] =
    Domain()

/** Base trait for CQRS SaaS services.
  *
  * Extend this instead of `CQRSModel#Service` to get automatic tenant isolation
  * and authorization guards. The raw `CQRSDomainDSL` is not accessible; only
  * the guarded `SaaS` DSL is exposed.
  *
  * @tparam Auth
  *   authentication/authorization context type
  * @tparam C
  *   business command type (will be wrapped in `SaaSCommand[Auth, C]`)
  * @tparam A
  *   business entity state type (will be wrapped in `CrudState[A]`)
  * @tparam R
  *   rejection type
  * @tparam N
  *   notification type
  */
abstract class SaaSCQRSService[Auth, C, A, R, N](
    mkRejection: String => R
)(using StateModelTC[CrudState[A]], AuthPolicy[Auth]):

  protected final val SaaS: SaaSCQRSDomainDSL[Auth, C, A, R, N] =
    SaaSCQRSDomainDSL(mkRejection)

  final type App[F[_], T] = SaaS.App[F, T]

  final type Handler[F[_]] =
    DomainService[F, CommandMessage[SaaSCommand[Auth, C]], R]

  final val domain: CQRSDomain[SaaSCommand[Auth, C], CrudState[A], R, N] =
    CQRSDomain()
