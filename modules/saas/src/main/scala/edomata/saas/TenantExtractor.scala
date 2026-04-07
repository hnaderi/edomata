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

/** Typeclass for extracting tenant and owner identity from a state value.
  *
  * Used by the SaaS-aware backend drivers to populate `tenant_id` and
  * `owner_id` columns in the database tables.
  */
trait TenantExtractor[S]:
  def extract(s: S): Option[(TenantId, UserId)]

object TenantExtractor:
  def apply[S](using te: TenantExtractor[S]): TenantExtractor[S] = te

  given [A]: TenantExtractor[CrudState[A]] with
    def extract(s: CrudState[A]): Option[(TenantId, UserId)] = s match
      case CrudState.Active(tid, oid, _) => Some((tid, oid))
      case CrudState.Deleted(tid, oid)   => Some((tid, oid))
      case CrudState.NonExistent         => None
