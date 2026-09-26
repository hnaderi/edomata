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

package edomata.backend

import java.nio.charset.StandardCharsets
import java.nio.file.Files
import java.nio.file.Paths

/** Generates the golden DDL files asserted by the Rust port
  * (`rust/crates/edomata-postgres/tests/golden.rs`).
  *
  * Usage (from the repository root):
  * {{{
  * sbt "postgresJVM/Test/runMain edomata.backend.GoldenDDL rust/tests/golden"
  * }}}
  *
  * Each file is the DDL statement list joined by `\n` plus a trailing newline.
  * The cases must stay in sync with `NAMINGS`, `ES_TYPES` and `CQRS_TYPES` in
  * the Rust test.
  */
object GoldenDDL {
  private val namings: List[(String, String)] = List(
    "schema" -> "auth",
    "prefixed" -> "myapp",
    "schema" -> "Order_v2$",
    "prefixed" -> "order_v2$"
  )

  private val esTypes: List[(String, String, String, String)] = List(
    ("jsonb", "jsonb", "jsonb", "jsonb"),
    ("json", "json", "json", "json"),
    ("bytea", "bytea", "bytea", "bytea"),
    ("mixed", "bytea", "json", "jsonb")
  )

  private val cqrsTypes: List[(String, String, String)] = List(
    ("jsonb", "jsonb", "jsonb"),
    ("json", "json", "json"),
    ("bytea", "bytea", "bytea"),
    ("mixed", "json", "bytea")
  )

  private def naming(kind: String, ns: String): PGNaming = {
    val namespace = PGNamespace.fromString(ns).fold(sys.error, identity)
    kind match {
      case "schema"   => PGNaming.schema(namespace)
      case "prefixed" => PGNaming.prefixed(namespace)
      case other      => sys.error(s"unknown naming $other")
    }
  }

  private def fileName(
      prefix: String,
      kind: String,
      ns: String,
      label: String
  ) =
    s"${prefix}_${kind}_${ns.toLowerCase.replace('$', '_')}_$label.sql"

  def main(args: Array[String]): Unit = {
    val dir = Paths.get(args.headOption.getOrElse("rust/tests/golden"))
    Files.createDirectories(dir)
    def write(name: String, ddl: List[String]): Unit = {
      val content = ddl.mkString("\n") + "\n"
      Files.write(dir.resolve(name), content.getBytes(StandardCharsets.UTF_8))
      println(s"wrote ${dir.resolve(name)}")
    }
    for ((kind, ns) <- namings) {
      val n = naming(kind, ns)
      for ((label, ev, notif, snap) <- esTypes)
        write(
          fileName("eventsourcing", kind, ns, label),
          PGSchema.eventsourcing(n, ev, notif, snap)
        )
      for ((label, state, notif) <- cqrsTypes)
        write(fileName("cqrs", kind, ns, label), PGSchema.cqrs(n, state, notif))
    }
  }
}
