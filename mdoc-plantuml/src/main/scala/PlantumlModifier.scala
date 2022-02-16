package ir.hnaderi.mdocPlantuml

import mdoc.Reporter
import mdoc.StringModifier
import net.sourceforge.plantuml.FileFormat
import net.sourceforge.plantuml.FileFormatOption
import net.sourceforge.plantuml.SourceStringReader

import scala.meta.inputs.Input

class PlantumlModifier extends StringModifier {
  val name: String = "plantuml"
  def process(info: String, code: Input, reporter: Reporter): String =
    val fmt = if info.isEmpty then "svg" else info.toLowerCase
    val input = code.text
    val ssr = new SourceStringReader(s"@startuml\n$input \n@enduml")
    val enc = ssr.getBlocks.get(0).getEncodedUrl

    s"![](https://plantuml.com/plantuml/$fmt/$enc)\n"
}
