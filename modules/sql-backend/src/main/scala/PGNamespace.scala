package edomata.backend

import scala.util.matching.Regex

opaque type PGNamespace <: String = String

object PGNamespace {
  import scala.compiletime
  import scala.quoted.*

  private def toNs(ns: Expr[String])(using ctx: Quotes): Expr[PGNamespace] = {
    import ctx.reflect.report.errorAndAbort
    val str =
      ns.value.getOrElse(errorAndAbort("Must provide a literal constant"))
    fromString(str).fold(
      errorAndAbort,
      Expr(_)
    )
  }

  inline def apply(inline ns: String): PGNamespace = ${ toNs('ns) }

  private val maxLen = 63
  private val pat: Regex = "([A-Za-z_][A-Za-z_0-9$]*)".r

  def fromString(s: String): Either[String, PGNamespace] =
    s match {
      case pat(s) =>
        if (s.length > maxLen)
          Left(s"Name is too long: ${s.length} (max allowed is $maxLen)")
        else
          Right(s)
      case _ => Left(s"Name: \"$s\" does not match ${pat.regex}")
    }
}
