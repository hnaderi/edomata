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

package ir.hnaderi.mdocPlantuml

import mdoc.Reporter
import mdoc.StringModifier
import net.sourceforge.plantuml.SourceStringReader

import scala.meta.inputs.Input

class PlantumlModifier extends StringModifier {
  val name: String = "plantuml"
  def process(info: String, code: Input, reporter: Reporter): String =
    val directive = if info.isEmpty then "uml" else info.toLowerCase
    val input = code.text
    val ssr = new SourceStringReader(s"""
@start$directive
!theme toy

$input
@end$directive
""")
    val enc = ssr.getBlocks.get(0).getEncodedUrl

    s"![](https://www.plantuml.com/plantuml/svg/$enc)  \n"
}
