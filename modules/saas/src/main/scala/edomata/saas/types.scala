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

opaque type TenantId = String
object TenantId:
  def apply(value: String): TenantId = value
  extension (t: TenantId) def value: String = t

opaque type UserId = String
object UserId:
  def apply(value: String): UserId = value
  extension (u: UserId) def value: String = u

/** Typeclass that defines how to extract tenant identity and verify
  * authorization from a generic auth context.
  *
  * Implement this for your own auth type (JWT claims, API key context, etc.) to
  * plug into the SaaS guard system.
  *
  * @tparam Auth
  *   the authentication/authorization context type
  */
trait AuthPolicy[Auth]:
  /** Extract the tenant identifier from the auth context */
  def tenantId(auth: Auth): TenantId

  /** Verify that the auth context is authorized to perform the given action.
    * Return `Right(())` if allowed, `Left(reason)` if denied.
    */
  def authorize(auth: Auth, action: CrudAction): Either[String, Unit]

/** Default auth context with role-based authorization.
  *
  * This is provided as a convenient default. You can define your own auth type
  * and implement `AuthPolicy` for it instead.
  */
final case class CallerIdentity(
    tenantId: TenantId,
    userId: UserId,
    roles: Set[String]
)

object CallerIdentity:
  given AuthPolicy[CallerIdentity] = new AuthPolicy[CallerIdentity]:
    def tenantId(auth: CallerIdentity): TenantId = auth.tenantId

    def authorize(
        auth: CallerIdentity,
        action: CrudAction
    ): Either[String, Unit] =
      // CallerIdentity alone doesn't know which roles are required;
      // this default policy always allows. Use RoleBasedPolicy for
      // role checking.
      Right(())

/** Role-based auth policy for CallerIdentity with configurable role mappings.
  */
final class RoleBasedPolicy(rolesFor: CrudAction => Set[String])
    extends AuthPolicy[CallerIdentity]:
  def tenantId(auth: CallerIdentity): TenantId = auth.tenantId

  def authorize(
      auth: CallerIdentity,
      action: CrudAction
  ): Either[String, Unit] =
    val required = rolesFor(action)
    val missing = required -- auth.roles
    if missing.isEmpty then Right(())
    else Left(s"Missing roles: ${missing.mkString(", ")}")

/** Wraps any command C with an auth context -- used as CommandMessage payload
  * type.
  *
  * @tparam Auth
  *   authentication/authorization context type
  * @tparam C
  *   business command type
  */
final case class SaaSCommand[Auth, +C](
    auth: Auth,
    payload: C
)

/** CRUD action enum for permission mapping */
enum CrudAction:
  case Create, Read, Update, Delete
