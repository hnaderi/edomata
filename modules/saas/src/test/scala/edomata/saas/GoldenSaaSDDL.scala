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

import edomata.backend.PGNaming
import edomata.backend.PGNamespace

import java.nio.charset.StandardCharsets
import java.nio.file.Files
import java.nio.file.Paths

/** Generates the golden SaaS DDL files asserted by the Rust port
  * (`rust/crates/edomata-saas/tests/schema.rs`).
  *
  * Usage (from the repository root):
  * {{{
  * sbt "saasJVM/Test/runMain edomata.saas.GoldenSaaSDDL rust/tests/golden"
  * }}}
  *
  * The cases must stay in sync with `NAMINGS`, `TYPES` and the RLS cases of
  * `saas_ddl_matches_scala_golden_files` in the Rust test.
  */
object GoldenSaaSDDL {
  private val namings: List[(String, String)] = List(
    "schema" -> "catalog",
    "prefixed" -> "catalog",
    "schema" -> "Shop_v2$"
  )

  private val types: List[(String, String, String)] = List(
    ("jsonb", "jsonb", "jsonb"),
    ("json", "json", "json"),
    ("bytea", "bytea", "bytea"),
    ("mixed", "bytea", "json")
  )

  private val rls: List[(String, Option[SaaSPGSchema.RLSConfig])] = List(
    "norls" -> None,
    "rls" -> Some(SaaSPGSchema.RLSConfig("app_user", "app.tenant_id"))
  )

  private def naming(kind: String, ns: String): PGNaming = {
    val namespace = PGNamespace.fromString(ns).fold(sys.error, identity)
    kind match {
      case "schema"   => PGNaming.schema(namespace)
      case "prefixed" => PGNaming.prefixed(namespace)
      case other      => sys.error(s"unknown naming $other")
    }
  }

  def main(args: Array[String]): Unit = {
    val dir = Paths.get(args.headOption.getOrElse("rust/tests/golden"))
    Files.createDirectories(dir)
    for {
      (kind, ns) <- namings
      (label, stateType, notifType) <- types
      (rlsLabel, config) <- rls
    } {
      val n = naming(kind, ns)
      val name =
        s"saas_cqrs_${kind}_${ns.toLowerCase.replace('$', '_')}_${label}_$rlsLabel.sql"
      val ddl = SaaSPGSchema.cqrs(n, stateType, notifType, config)
      val content = ddl.mkString("\n") + "\n"
      Files.write(dir.resolve(name), content.getBytes(StandardCharsets.UTF_8))
      println(s"wrote ${dir.resolve(name)}")
    }
  }
}
