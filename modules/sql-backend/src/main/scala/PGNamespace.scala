package edomata.backend

opaque type PGNamespace <: String = String

object PGNamespace {
  import scala.compiletime
  import scala.quoted.*

  private def toNs(ns: Expr[String])(using ctx: Quotes): Expr[PGNamespace] = {
    import ctx.reflect.report.errorAndAbort
    val str =
      ns.value.getOrElse(errorAndAbort("Must provide a literal constant"))
    if str.contains("-") then errorAndAbort(s"Invalid namespace $str!")
    else Expr(str)
  }

  inline def apply(inline ns: String): PGNamespace = ${ toNs('ns) }
}
