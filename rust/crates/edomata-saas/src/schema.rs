//! DDL for tenant-aware CQRS tables and for the event-sourced tables under
//! mandatory Row-Level Security.

use edomata_postgres::PGNaming;

use crate::rls::{AccessMode, RlsAccessPolicy, RlsPolicyError, check_identifier};

/// PostgreSQL Row-Level Security configuration for
/// [`SaaSPGSchema::cqrs_with`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RlsConfig {
    /// The PostgreSQL role to grant access to (e.g. `"app_user"`).
    pub pg_role: String,
    /// The session variable holding the current tenant id (e.g.
    /// `"app.tenant_id"`).
    pub tenant_session_var: String,
}

impl RlsConfig {
    /// Builds a configuration.
    pub fn new(pg_role: impl Into<String>, tenant_session_var: impl Into<String>) -> Self {
        Self {
            pg_role: pg_role.into(),
            tenant_session_var: tenant_session_var.into(),
        }
    }
}

/// DDL for SaaS-aware CQRS tables. Unlike `PGSchema`, the states and outbox
/// tables carry `tenant_id` (and `owner_id`) columns, enabling Row-Level
/// Security, tenant-scoped indexes and cross-tenant audit queries. The
/// output is byte-for-byte identical to Scala's `SaaSPGSchema`.
///
/// ```
/// use edomata_saas::{PGNaming, RlsConfig, SaaSPGSchema};
///
/// let naming = PGNaming::prefixed_str("catalog").unwrap();
/// let ddl = SaaSPGSchema::cqrs_with(&naming, "jsonb", "jsonb", Some(&RlsConfig::new("app_user", "app.tenant_id")));
/// assert!(ddl.iter().any(|s| s.contains("ENABLE ROW LEVEL SECURITY")));
/// assert!(ddl.iter().any(|s| s.contains("USING (tenant_id = current_setting('app.tenant_id'))")));
/// assert!(ddl.iter().any(|s| s.starts_with("GRANT SELECT, INSERT, UPDATE ON catalog_states TO app_user")));
/// ```
///
/// With RLS, the policies compare `tenant_id` with the session variable, so
/// connections using `pg_role` must set it (`SET app.tenant_id = 'acme'`,
/// or `set_config` per transaction) before touching the tables. Unlike the
/// table statements, `CREATE POLICY` has no `IF NOT EXISTS`: run the RLS
/// statements once, from a migration tool.
#[derive(Clone, Copy, Debug, Default)]
pub struct SaaSPGSchema;

impl SaaSPGSchema {
    /// DDL for states (with `tenant_id`, `owner_id`), outbox (with
    /// `tenant_id`) and commands, `jsonb` payloads, no RLS. In schema naming
    /// mode the first statement is `CREATE SCHEMA IF NOT EXISTS`.
    pub fn cqrs(naming: &PGNaming) -> Vec<String> {
        Self::cqrs_with(naming, "jsonb", "jsonb", None)
    }

    /// DDL with explicit payload types and optional RLS statements
    /// (appended last). Payload types are spliced into the DDL without
    /// validation; use `"jsonb"`, `"json"` or `"bytea"`.
    pub fn cqrs_with(
        naming: &PGNaming,
        state_type: &str,
        notification_type: &str,
        rls: Option<&RlsConfig>,
    ) -> Vec<String> {
        let mut out = edomata_postgres::ddl::schema_statement(naming);
        out.extend(ddl::states_statements(naming, state_type));
        out.extend(ddl::outbox_statements(naming, notification_type));
        out.extend(edomata_postgres::ddl::commands_statements(naming));
        out.extend(ddl::rls_statements(naming, rls));
        out
    }

    /// DDL of the **event-sourced** tables (`journal`, `outbox`,
    /// `commands`, `snapshots`) under mandatory Row-Level Security: the
    /// tables with their generated `tenant_id` column
    /// ([`eventsourcing_rls_tables`](Self::eventsourcing_rls_tables)) followed
    /// by the RLS statements (`ENABLE` and `FORCE`, the three policies per
    /// table, the grants). Payload types are spliced as is (`jsonb`, `json`
    /// or `bytea`).
    ///
    /// Run it **once, from a migration tool, as the owner of the tables**
    /// (a role the application does not use): `CREATE POLICY` has no `IF
    /// NOT EXISTS`, so applying it twice fails; the table statements are
    /// idempotent. Create the roles before
    /// ([`rls_roles`](Self::rls_roles)) and grant the login roles
    /// afterwards ([`rls_memberships`](Self::rls_memberships)).
    ///
    /// ```
    /// use edomata_saas::{PGNaming, RlsAccessPolicy, SaaSPGSchema, TenantId};
    ///
    /// let naming = PGNaming::prefixed_str("shop").unwrap();
    /// let policy: RlsAccessPolicy<TenantId> = RlsAccessPolicy::with_prefix("shop").unwrap();
    /// let ddl = SaaSPGSchema::eventsourcing_rls(&naming, "jsonb", "jsonb", "jsonb", &policy);
    /// assert!(ddl.iter().any(|s| s == "ALTER TABLE shop_journal FORCE ROW LEVEL SECURITY;"));
    /// assert!(ddl.iter().any(|s| s.contains("CREATE POLICY shop_journal_tenant_policy")));
    /// ```
    pub fn eventsourcing_rls<T>(
        naming: &PGNaming,
        event_type: &str,
        notification_type: &str,
        snapshot_type: &str,
        policy: &RlsAccessPolicy<T>,
    ) -> Vec<String> {
        let mut out =
            Self::eventsourcing_rls_tables(naming, event_type, notification_type, snapshot_type);
        out.extend(ddl::es_rls_statements(naming, policy));
        out
    }

    /// Only the (idempotent) table statements of
    /// [`eventsourcing_rls`](Self::eventsourcing_rls): `journal`,
    /// `outbox`, `commands` and `snapshots`, each with a `tenant_id` column
    /// **generated** from the tenant-prefixed stream or command id, and the
    /// indexes. The `migrations` and `relay_checkpoints` tables hold no
    /// tenant data and are not part of it (append
    /// [`edomata_postgres::PGSchema::relay_checkpoints`] or
    /// `ddl::migrations_statements` when you use them).
    pub fn eventsourcing_rls_tables(
        naming: &PGNaming,
        event_type: &str,
        notification_type: &str,
        snapshot_type: &str,
    ) -> Vec<String> {
        let mut out = edomata_postgres::ddl::schema_statement(naming);
        out.extend(ddl::es_journal_statements(naming, event_type));
        out.extend(ddl::es_outbox_statements(naming, notification_type));
        out.extend(ddl::es_commands_statements(naming));
        out.extend(ddl::es_snapshots_statements(naming, snapshot_type));
        out
    }

    /// Statements creating the three `NOLOGIN` roles of `policy` unless they
    /// exist (idempotent; roles are cluster-wide, so a DBA usually runs
    /// them, not the migration of one service).
    pub fn rls_roles<T>(policy: &RlsAccessPolicy<T>) -> Vec<String> {
        [
            policy.tenant_role(),
            policy.system_role(),
            policy.inspection_role(),
        ]
        .into_iter()
        .map(|role| {
            format!(
                "DO $$ BEGIN IF NOT EXISTS (SELECT FROM pg_roles WHERE rolname = '{role}') THEN CREATE ROLE {role} NOLOGIN NOSUPERUSER NOBYPASSRLS; END IF; END $$;"
            )
        })
        .collect()
    }

    /// `GRANT role TO login` for each of `modes`: the **membership** that
    /// lets connections of `login` `SET LOCAL ROLE` to those roles. Grant
    /// each login only the modes it needs: the web tier's login gets
    /// [`AccessMode::Tenant`], a relay's login [`AccessMode::System`], an
    /// inspection agent's login [`AccessMode::Inspection`]. Create the
    /// login with `NOINHERIT`, so that it has no access without the explicit
    /// `SET LOCAL ROLE` of the entry point.
    ///
    /// # Errors
    ///
    /// [`RlsPolicyError::InvalidIdentifier`] if `login` is not a lowercase
    /// identifier.
    ///
    /// ```
    /// use edomata_saas::{AccessMode, RlsAccessPolicy, SaaSPGSchema, TenantId};
    ///
    /// let policy: RlsAccessPolicy<TenantId> = RlsAccessPolicy::with_prefix("shop").unwrap();
    /// let sql = SaaSPGSchema::rls_memberships(&policy, "shop_web", &[AccessMode::Tenant]).unwrap();
    /// assert_eq!(sql, vec!["GRANT shop_tenant TO shop_web;".to_string()]);
    /// ```
    pub fn rls_memberships<T>(
        policy: &RlsAccessPolicy<T>,
        login: &str,
        modes: &[AccessMode],
    ) -> Result<Vec<String>, RlsPolicyError> {
        check_identifier(login)?;
        Ok(modes
            .iter()
            .map(|mode| format!("GRANT {} TO {login};", policy.role(*mode)))
            .collect())
    }
}

/// The individual tenant-aware DDL statements, reused by the SaaS driver.
pub mod ddl {
    use edomata_postgres::PGNaming;

    use super::{RlsAccessPolicy, RlsConfig};

    /// The states table with tenant and owner columns and indexes.
    pub fn states_statements(naming: &PGNaming, payload_type: &str) -> Vec<String> {
        let t = naming.table("states");
        let pk = naming.constraint("states_pk");
        let tenant_idx = naming.index("states_tenant_idx");
        let tenant_owner_idx = naming.index("states_tenant_owner_idx");
        vec![
            format!(
                "CREATE TABLE IF NOT EXISTS {t} (\n  id text NOT NULL,\n  \"version\" int8 NOT NULL,\n  state {payload_type} NOT NULL,\n  tenant_id text NOT NULL,\n  owner_id text NOT NULL,\n  CONSTRAINT {pk} PRIMARY KEY (id)\n);"
            ),
            format!("CREATE INDEX IF NOT EXISTS {tenant_idx} ON {t} (tenant_id);"),
            format!("CREATE INDEX IF NOT EXISTS {tenant_owner_idx} ON {t} (tenant_id, owner_id);"),
        ]
    }

    /// The outbox table with a tenant column and index.
    pub fn outbox_statements(naming: &PGNaming, payload_type: &str) -> Vec<String> {
        let t = naming.table("outbox");
        let pk = naming.constraint("outbox_pk");
        let tenant_idx = naming.index("outbox_tenant_idx");
        vec![
            format!(
                "CREATE TABLE IF NOT EXISTS {t} (\n  seqnr bigserial NOT NULL,\n  stream text NOT NULL,\n  correlation text NULL,\n  causation text NULL,\n  payload {payload_type} NOT NULL,\n  created timestamptz NOT NULL,\n  published timestamptz NULL,\n  tenant_id text NOT NULL,\n  CONSTRAINT {pk} PRIMARY KEY (seqnr)\n);"
            ),
            format!("CREATE INDEX IF NOT EXISTS {tenant_idx} ON {t} (tenant_id);"),
        ]
    }

    /// Row-Level Security policies and grants, when configured (empty for
    /// `None`).
    pub fn rls_statements(naming: &PGNaming, rls: Option<&RlsConfig>) -> Vec<String> {
        let Some(config) = rls else {
            return Vec::new();
        };
        let states_t = naming.table("states");
        let outbox_t = naming.table("outbox");
        let states_policy = naming.constraint("states_tenant_policy");
        let outbox_policy = naming.constraint("outbox_tenant_policy");
        let var = &config.tenant_session_var;
        let role = &config.pg_role;
        vec![
            format!("ALTER TABLE {states_t} ENABLE ROW LEVEL SECURITY;"),
            format!(
                "CREATE POLICY {states_policy} ON {states_t}\n  USING (tenant_id = current_setting('{var}'));"
            ),
            format!("GRANT SELECT, INSERT, UPDATE ON {states_t} TO {role};"),
            format!("ALTER TABLE {outbox_t} ENABLE ROW LEVEL SECURITY;"),
            format!(
                "CREATE POLICY {outbox_policy} ON {outbox_t}\n  USING (tenant_id = current_setting('{var}'));"
            ),
            format!("GRANT SELECT, INSERT, UPDATE ON {outbox_t} TO {role};"),
        ]
    }

    /// The generated `tenant_id` expression over a tenant-prefixed id
    /// column: the text before the first `/`, and NULL (so the NOT NULL
    /// constraint refuses the row) when there is no `/` or the tenant is
    /// empty.
    fn tenant_of(column: &str) -> String {
        format!("CASE WHEN position('/' in {column}) > 1 THEN split_part({column}, '/', 1) END")
    }

    fn tenant_column(column: &str) -> String {
        format!(
            "  tenant_id text GENERATED ALWAYS AS ({}) STORED NOT NULL",
            tenant_of(column)
        )
    }

    /// The event-sourced journal with a `tenant_id` column generated from
    /// `stream`.
    pub fn es_journal_statements(naming: &PGNaming, payload_type: &str) -> Vec<String> {
        let t = naming.table("journal");
        let pk = naming.constraint("journal_pk");
        let un = naming.constraint("journal_un");
        let seqnr_idx = naming.index("journal_seqnr_idx");
        let stream_idx = naming.index("journal_stream_idx");
        let tenant_idx = naming.index("journal_tenant_idx");
        let tenant = tenant_column("stream");
        vec![
            format!(
                "CREATE TABLE IF NOT EXISTS {t} (\n  id uuid NOT NULL,\n  \"time\" timestamptz NOT NULL,\n  seqnr bigserial NOT NULL,\n  \"version\" int8 NOT NULL,\n  stream text NOT NULL,\n  payload {payload_type} NOT NULL,\n{tenant},\n  CONSTRAINT {pk} PRIMARY KEY (id),\n  CONSTRAINT {un} UNIQUE (stream, version)\n);"
            ),
            format!("CREATE INDEX IF NOT EXISTS {seqnr_idx} ON {t} USING btree (seqnr);"),
            format!(
                "CREATE INDEX IF NOT EXISTS {stream_idx} ON {t} USING btree (stream, version);"
            ),
            format!(
                "CREATE INDEX IF NOT EXISTS {tenant_idx} ON {t} USING btree (tenant_id, seqnr);"
            ),
        ]
    }

    /// The event-sourced outbox with a `tenant_id` column generated from
    /// `stream`.
    pub fn es_outbox_statements(naming: &PGNaming, payload_type: &str) -> Vec<String> {
        let t = naming.table("outbox");
        let pk = naming.constraint("outbox_pk");
        let tenant_idx = naming.index("outbox_tenant_idx");
        let tenant = tenant_column("stream");
        vec![
            format!(
                "CREATE TABLE IF NOT EXISTS {t} (\n  seqnr bigserial NOT NULL,\n  stream text NOT NULL,\n  correlation text NULL,\n  causation text NULL,\n  payload {payload_type} NOT NULL,\n  created timestamptz NOT NULL,\n  published timestamptz NULL,\n{tenant},\n  CONSTRAINT {pk} PRIMARY KEY (seqnr)\n);"
            ),
            format!("CREATE INDEX IF NOT EXISTS {tenant_idx} ON {t} (tenant_id);"),
        ]
    }

    /// The commands table with a `tenant_id` column generated from `id`,
    /// and a check that `address` belongs to the same tenant as `id`.
    pub fn es_commands_statements(naming: &PGNaming) -> Vec<String> {
        let t = naming.table("commands");
        let pk = naming.constraint("commands_pk");
        let ck = naming.constraint("commands_address_tenant_ck");
        let tenant_idx = naming.index("commands_tenant_idx");
        let tenant = tenant_column("id");
        vec![
            format!(
                "CREATE TABLE IF NOT EXISTS {t} (\n  id text NOT NULL,\n  \"time\" timestamptz NOT NULL,\n  address text NOT NULL,\n{tenant},\n  CONSTRAINT {pk} PRIMARY KEY (id),\n  CONSTRAINT {ck} CHECK (position('/' in address) > 1 AND split_part(address, '/', 1) = tenant_id)\n);"
            ),
            format!("CREATE INDEX IF NOT EXISTS {tenant_idx} ON {t} (tenant_id);"),
        ]
    }

    /// The snapshots table with a `tenant_id` column generated from `id`.
    pub fn es_snapshots_statements(naming: &PGNaming, payload_type: &str) -> Vec<String> {
        let t = naming.table("snapshots");
        let pk = naming.constraint("snapshots_pk");
        let tenant_idx = naming.index("snapshots_tenant_idx");
        let tenant = tenant_column("id");
        vec![
            format!(
                "CREATE TABLE IF NOT EXISTS {t} (\n  id text NOT NULL,\n  \"version\" int8 NOT NULL,\n  state {payload_type} NOT NULL,\n{tenant},\n  CONSTRAINT {pk} PRIMARY KEY (id)\n);"
            ),
            format!("CREATE INDEX IF NOT EXISTS {tenant_idx} ON {t} (tenant_id);"),
        ]
    }

    /// RLS enabled **and forced**, the three policies and the grants of
    /// each of the four event-sourced tables. Not idempotent (`CREATE
    /// POLICY`): run once from a migration tool, as the table owner.
    ///
    /// Per table: `<table>_tenant_policy` (the tenant role, `USING` and
    /// `WITH CHECK` on `tenant_id = NULLIF(current_setting(setting, true),
    /// '')`, so a missing or empty tenant matches nothing),
    /// `<table>_system_policy` (the system role, everything) and
    /// `<table>_inspection_policy` (the inspection role, `SELECT` only).
    /// Privileges are the minimum each role needs: the tenant role cannot
    /// update or delete journal, outbox or command rows, and the
    /// inspection role has `SELECT` only, so its writes fail on privileges
    /// before RLS.
    pub fn es_rls_statements<T>(naming: &PGNaming, policy: &RlsAccessPolicy<T>) -> Vec<String> {
        let tenant_role = policy.tenant_role();
        let system_role = policy.system_role();
        let inspection_role = policy.inspection_role();
        let tenant_check = format!(
            "tenant_id = NULLIF(current_setting('{}', true), '')",
            policy.tenant_setting()
        );
        // (table, tenant privileges, sequence)
        let tables: [(&str, &str, Option<&str>); 4] = [
            ("journal", "SELECT, INSERT", Some("journal_seqnr_seq")),
            ("outbox", "SELECT, INSERT", Some("outbox_seqnr_seq")),
            ("commands", "SELECT, INSERT", None),
            ("snapshots", "SELECT, INSERT, UPDATE", None),
        ];
        let mut out = Vec::new();
        if naming.needs_schema_setup() {
            // Table privileges are useless without the right to look names
            // up in the schema.
            out.push(format!(
                "GRANT USAGE ON SCHEMA \"{}\" TO {tenant_role}, {system_role}, {inspection_role};",
                naming.namespace()
            ));
        }
        for (name, tenant_privileges, sequence) in tables {
            let t = naming.table(name);
            let tenant_policy = naming.constraint(&format!("{name}_tenant_policy"));
            let system_policy = naming.constraint(&format!("{name}_system_policy"));
            let inspection_policy = naming.constraint(&format!("{name}_inspection_policy"));
            out.push(format!("ALTER TABLE {t} ENABLE ROW LEVEL SECURITY;"));
            out.push(format!("ALTER TABLE {t} FORCE ROW LEVEL SECURITY;"));
            out.push(format!(
                "CREATE POLICY {tenant_policy} ON {t} FOR ALL TO {tenant_role}\n  USING ({tenant_check})\n  WITH CHECK ({tenant_check});"
            ));
            out.push(format!(
                "CREATE POLICY {system_policy} ON {t} FOR ALL TO {system_role}\n  USING (true)\n  WITH CHECK (true);"
            ));
            out.push(format!(
                "CREATE POLICY {inspection_policy} ON {t} FOR SELECT TO {inspection_role}\n  USING (true);"
            ));
            out.push(format!(
                "GRANT {tenant_privileges} ON {t} TO {tenant_role};"
            ));
            out.push(format!(
                "GRANT SELECT, INSERT, UPDATE, DELETE ON {t} TO {system_role};"
            ));
            out.push(format!("GRANT SELECT ON {t} TO {inspection_role};"));
            if let Some(seq) = sequence {
                let seq = naming.table(seq);
                out.push(format!(
                    "GRANT USAGE ON SEQUENCE {seq} TO {tenant_role}, {system_role};"
                ));
            }
        }
        out
    }
}
