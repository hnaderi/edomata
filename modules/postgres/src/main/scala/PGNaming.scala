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

import scala.annotation.targetName

/** Naming strategy for PostgreSQL tables used by edomata backends.
  *
  * [[PGNaming.Schema]] (default) creates a dedicated PostgreSQL schema per
  * aggregate, e.g. `"auth".journal`.
  *
  * [[PGNaming.Prefixed]] uses a table-name prefix inside the current schema,
  * e.g. `auth_journal`, which is useful when a separate schema is undesirable
  * (for instance, with Flyway migrations in `public`).
  */
sealed trait PGNaming {

  /** The underlying namespace value. */
  def namespace: PGNamespace

  /** Whether a `CREATE SCHEMA` statement is needed during setup. */
  def needsSchemaSetup: Boolean

  /** Returns a fully-qualified (schema mode) or prefixed (prefix mode) table
    * reference suitable for embedding in SQL.
    */
  def table(name: String): String

  /** Returns a constraint name, optionally prefixed to avoid collisions when
    * multiple aggregates share the same schema.
    */
  def constraint(name: String): String

  /** Returns an index name, optionally prefixed to avoid collisions. */
  def index(name: String): String
}

object PGNaming {

  /** Schema-based naming: creates a dedicated PostgreSQL schema and references
    * tables as `"namespace".table_name`.
    */
  final case class Schema(namespace: PGNamespace) extends PGNaming {
    def needsSchemaSetup: Boolean = true
    def table(name: String): String = s""""$namespace".$name"""
    def constraint(name: String): String = name
    def index(name: String): String = name
  }

  /** Prefix-based naming: no schema creation; tables are named
    * `namespace_table_name` in the current schema.
    */
  final case class Prefixed(namespace: PGNamespace) extends PGNaming {
    def needsSchemaSetup: Boolean = false
    def table(name: String): String = s"${namespace}_$name"
    def constraint(name: String): String = s"${namespace}_$name"
    def index(name: String): String = s"${namespace}_$name"
  }

  /** Constructs schema-based naming from a validated [[PGNamespace]]. */
  def schema(ns: PGNamespace): PGNaming = Schema(ns)

  /** Constructs prefix-based naming from a validated [[PGNamespace]]. */
  def prefixed(ns: PGNamespace): PGNaming = Prefixed(ns)

  /** Constructs schema-based naming with compile-time validation. */
  @targetName("schemaFromString")
  inline def schema(inline ns: String): PGNaming = Schema(PGNamespace(ns))

  /** Constructs prefix-based naming with compile-time validation. */
  @targetName("prefixedFromString")
  inline def prefixed(inline ns: String): PGNaming = Prefixed(PGNamespace(ns))
}
