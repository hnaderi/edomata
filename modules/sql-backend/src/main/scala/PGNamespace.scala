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
