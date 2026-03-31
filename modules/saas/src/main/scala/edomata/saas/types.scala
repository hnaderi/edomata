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

opaque type TenantId = String
object TenantId:
  def apply(value: String): TenantId = value
  extension (t: TenantId) def value: String = t

opaque type UserId = String
object UserId:
  def apply(value: String): UserId = value
  extension (u: UserId) def value: String = u

final case class CallerIdentity(
    tenantId: TenantId,
    userId: UserId,
    roles: Set[String]
)

final case class SaaSCommand[+C](
    caller: CallerIdentity,
    payload: C
)

enum CrudAction:
  case Create, Read, Update, Delete
