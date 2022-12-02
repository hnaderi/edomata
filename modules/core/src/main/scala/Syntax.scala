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

package edomata
package syntax

object all extends AllSyntax

trait AllSyntax
    extends core.ModelSyntax,
      core.DecisionSyntax,
      core.EdomatonSyntax,
      core.DomainSyntax,
      core.CQRSDomainSyntax

object decision extends core.DecisionSyntax
object edomaton extends core.EdomatonSyntax
object domain extends core.DomainSyntax
object cqrs extends core.CQRSDomainSyntax
