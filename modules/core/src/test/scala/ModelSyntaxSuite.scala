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

package edomata.core

import cats.data.Validated
import edomata.syntax.all.*
import munit.FunSuite

import ModelSyntaxSuite.given

class ModelSyntaxSuite extends FunSuite {
  test(".accept(evs)") {
    assertEquals(1.accept(2), 1.perform(2.accept))
    assertEquals(1.accept(2), Decision.acceptReturn(3)(2))

    assertEquals(1.accept(2, 3), 1.perform(Decision.accept(2, 3)))
    assertEquals(1.accept(2, 3), Decision.acceptReturn(6)(2, 3))
  }
}

object ModelSyntaxSuite extends DomainModel[Int, Int, String] {
  def initial = 0
  def transition = i => s => Validated.condNec(i > 0, i + s, "Invalid number!")
}
