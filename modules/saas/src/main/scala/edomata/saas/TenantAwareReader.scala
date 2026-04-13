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

import cats.Monad

/** Single-entity read with mandatory tenant scoping.
  *
  * @tparam Auth
  *   authentication/authorization context type
  */
trait TenantAwareReader[F[_], Auth, A]:
  def get(auth: Auth, entityId: String): F[Option[A]]

/** List/search queries with mandatory tenant scoping.
  *
  * The `Auth` parameter is structurally required, making it impossible to
  * forget tenant filtering.
  */
trait TenantScopedQuery[F[_], Auth, A, Q]:
  def query(auth: Auth, q: Q): F[List[A]]

/** Unguarded cross-tenant query for super-admin / internal dashboards.
  *
  * The `unsafe` prefix flags usage in code reviews. Use only for administrative
  * endpoints that explicitly require cross-tenant access.
  */
trait UnsafeCrossTenantQuery[F[_], A, Q]:
  def query(q: Q): F[List[A]]

object TenantScopedQuery:
  def apply[F[_]: Monad, Auth, A, Q](
      run: (TenantId, Q) => F[List[A]]
  )(using policy: AuthPolicy[Auth]): TenantScopedQuery[F, Auth, A, Q] =
    (auth, q) => run(policy.tenantId(auth), q)

object UnsafeCrossTenantQuery:
  def apply[F[_]: Monad, A, Q](
      run: Q => F[List[A]]
  ): UnsafeCrossTenantQuery[F, A, Q] =
    q => run(q)
