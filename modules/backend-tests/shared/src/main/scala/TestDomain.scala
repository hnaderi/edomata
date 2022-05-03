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

package tests

import cats.implicits.*
import edomata.core.*
import edomata.syntax.all.*

object TestDomain extends DomainModel[Int, Int, String] {
  def initial = 0
  def transition = i => s => (i + s).validNec
}

val TestDomainModel = TestDomain.domain[Int, Int]
