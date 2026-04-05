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

object SaaSGuard:
  def checkTenant[Auth, A](
      state: CrudState[A],
      auth: Auth,
      action: CrudAction
  )(using policy: AuthPolicy[Auth]): Either[String, Unit] =
    action match
      case CrudAction.Create => Right(())
      case _                 =>
        state match
          case CrudState.NonExistent       => Left("Entity not found")
          case CrudState.Active(tid, _, _) =>
            if tid == policy.tenantId(auth) then Right(())
            else Left("Tenant mismatch")
          case CrudState.Deleted(tid, _) =>
            if tid == policy.tenantId(auth) then Right(())
            else Left("Tenant mismatch")

  def checkAuthorization[Auth](
      auth: Auth,
      action: CrudAction
  )(using policy: AuthPolicy[Auth]): Either[String, Unit] =
    policy.authorize(auth, action)
