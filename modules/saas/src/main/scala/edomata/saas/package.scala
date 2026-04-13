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

import edomata.backend.Backend as EdomataBackend

/** Re-exports for clean external project usage.
  *
  * External projects should `import edomata.saas.*` and NOT import
  * `edomata.core.*` directly. This ensures they use the guarded SaaS
  * abstractions instead of the raw, unguarded DSLs.
  */

type CommandMessage[+C] = edomata.core.CommandMessage[C]
val CommandMessage = edomata.core.CommandMessage

type MessageMetadata = edomata.core.MessageMetadata
val MessageMetadata = edomata.core.MessageMetadata

type Decision[+R, +E, +A] = edomata.core.Decision[R, E, A]
val Decision = edomata.core.Decision

val Backend = EdomataBackend
