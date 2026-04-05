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

/** Generates DDL SQL statements for SaaS-aware edomata tables.
  *
  * Unlike [[edomata.backend.PGSchema]], these tables include `tenant_id` and
  * `owner_id` columns as first-class citizens, enabling:
  *   - PostgreSQL Row-Level Security (RLS)
  *   - Tenant-scoped indexes for query performance
  *   - SQL-level audit queries across tenants
  *
  * {{{
  * val ddl = SaaSPGSchema.cqrs(PGNaming.prefixed("catalog"))
  * ddl.foreach(println)
  * }}}
  */
object SaaSPGSchema {

  /** Configuration for PostgreSQL Row-Level Security.
    *
    * @param pgRole
    *   the PostgreSQL role to grant access to (e.g. "app_user")
    * @param tenantSessionVar
    *   the session variable holding the current tenant ID (e.g.
    *   "app.tenant_id")
    */
  final case class RLSConfig(
      pgRole: String,
      tenantSessionVar: String
  )

  /** Generates DDL for SaaS-aware CQRS tables: states (with tenant_id,
    * owner_id), outbox (with tenant_id), and commands.
    *
    * @param naming
    *   the naming strategy (schema or prefixed)
    * @param stateType
    *   PostgreSQL type for state payloads (default: `"jsonb"`)
    * @param notificationType
    *   PostgreSQL type for notification payloads (default: `"jsonb"`)
    * @param rls
    *   optional Row-Level Security configuration
    */
  def cqrs(
      naming: PGNaming,
      stateType: String = "jsonb",
      notificationType: String = "jsonb",
      rls: Option[RLSConfig] = None
  ): List[String] =
    schemaStatement(naming) ++
      statesStatements(naming, stateType) ++
      outboxStatements(naming, notificationType) ++
      commandsStatements(naming) ++
      rlsStatements(naming, rls)

  private def schemaStatement(naming: PGNaming): List[String] =
    if naming.needsSchemaSetup then
      List(s"""CREATE SCHEMA IF NOT EXISTS "${naming.namespace}";""")
    else Nil

  private def statesStatements(
      naming: PGNaming,
      payloadType: String
  ): List[String] = {
    val t = naming.table("states")
    val pk = naming.constraint("states_pk")
    val tenantIdx = naming.index("states_tenant_idx")
    val tenantOwnerIdx = naming.index("states_tenant_owner_idx")
    List(
      s"""CREATE TABLE IF NOT EXISTS $t (
         |  id text NOT NULL,
         |  "version" int8 NOT NULL,
         |  state $payloadType NOT NULL,
         |  tenant_id text NOT NULL,
         |  owner_id text NOT NULL,
         |  CONSTRAINT $pk PRIMARY KEY (id)
         |);""".stripMargin,
      s"CREATE INDEX IF NOT EXISTS $tenantIdx ON $t (tenant_id);",
      s"CREATE INDEX IF NOT EXISTS $tenantOwnerIdx ON $t (tenant_id, owner_id);"
    )
  }

  private def outboxStatements(
      naming: PGNaming,
      payloadType: String
  ): List[String] = {
    val t = naming.table("outbox")
    val pk = naming.constraint("outbox_pk")
    val tenantIdx = naming.index("outbox_tenant_idx")
    List(
      s"""CREATE TABLE IF NOT EXISTS $t (
         |  seqnr bigserial NOT NULL,
         |  stream text NOT NULL,
         |  correlation text NULL,
         |  causation text NULL,
         |  payload $payloadType NOT NULL,
         |  created timestamptz NOT NULL,
         |  published timestamptz NULL,
         |  tenant_id text NOT NULL,
         |  CONSTRAINT $pk PRIMARY KEY (seqnr)
         |);""".stripMargin,
      s"CREATE INDEX IF NOT EXISTS $tenantIdx ON $t (tenant_id);"
    )
  }

  private def commandsStatements(naming: PGNaming): List[String] = {
    val t = naming.table("commands")
    val pk = naming.constraint("commands_pk")
    List(
      s"""CREATE TABLE IF NOT EXISTS $t (
         |  id text NOT NULL,
         |  "time" timestamptz NOT NULL,
         |  address text NOT NULL,
         |  CONSTRAINT $pk PRIMARY KEY (id)
         |);""".stripMargin
    )
  }

  private def rlsStatements(
      naming: PGNaming,
      rls: Option[RLSConfig]
  ): List[String] =
    rls.toList.flatMap { config =>
      val statesT = naming.table("states")
      val outboxT = naming.table("outbox")
      val statesPolicy = naming.constraint("states_tenant_policy")
      val outboxPolicy = naming.constraint("outbox_tenant_policy")
      List(
        s"ALTER TABLE $statesT ENABLE ROW LEVEL SECURITY;",
        s"""CREATE POLICY $statesPolicy ON $statesT
           |  USING (tenant_id = current_setting('${config.tenantSessionVar}'));""".stripMargin,
        s"GRANT SELECT, INSERT, UPDATE ON $statesT TO ${config.pgRole};",
        s"ALTER TABLE $outboxT ENABLE ROW LEVEL SECURITY;",
        s"""CREATE POLICY $outboxPolicy ON $outboxT
           |  USING (tenant_id = current_setting('${config.tenantSessionVar}'));""".stripMargin,
        s"GRANT SELECT, INSERT, UPDATE ON $outboxT TO ${config.pgRole};"
      )
    }
}
