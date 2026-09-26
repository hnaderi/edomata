//! DDL for tenant-aware CQRS tables.

use edomata_postgres::PGNaming;

/// PostgreSQL Row-Level Security configuration.
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
/// ```
#[derive(Clone, Copy, Debug, Default)]
pub struct SaaSPGSchema;

impl SaaSPGSchema {
    /// DDL for states (with `tenant_id`, `owner_id`), outbox (with
    /// `tenant_id`) and commands, `jsonb` payloads, no RLS.
    pub fn cqrs(naming: &PGNaming) -> Vec<String> {
        Self::cqrs_with(naming, "jsonb", "jsonb", None)
    }

    /// DDL with explicit payload types and optional RLS statements.
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
}

/// The individual tenant-aware DDL statements, reused by the SaaS driver.
pub mod ddl {
    use edomata_postgres::PGNaming;

    use super::RlsConfig;

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

    /// Row-Level Security policies and grants, when configured.
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
}
