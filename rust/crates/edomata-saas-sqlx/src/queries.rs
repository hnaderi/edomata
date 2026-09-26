//! Tenant-aware SQL statements, mirroring `SaaSQueries.scala`.

use edomata_postgres::PGNaming;
use edomata_saas::ddl;

/// States table with `tenant_id` and `owner_id`.
pub(crate) struct SaaSStateQueries {
    pub setup: Vec<String>,
    pub get: String,
    /// Parameters: `(id, state, tenant_id, owner_id, expected_version)`.
    pub put: String,
    pub list_by_tenant: String,
}

impl SaaSStateQueries {
    pub fn new(naming: &PGNaming, payload_type: &str) -> Self {
        let t = naming.table("states");
        Self {
            setup: ddl::states_statements(naming, payload_type),
            get: format!("select state, version from {t} where id = $1"),
            put: format!(
                "insert into {t} (id, state, \"version\", tenant_id, owner_id) values ($1, $2, 1, $3, $4) on conflict (id) do update set version = {t}.version + 1, state = excluded.state, tenant_id = excluded.tenant_id, owner_id = excluded.owner_id where {t}.version = $5"
            ),
            list_by_tenant: format!("select state, version from {t} where tenant_id = $1"),
        }
    }
}

/// Outbox table with `tenant_id`.
pub(crate) struct SaaSOutboxQueries {
    pub setup: Vec<String>,
    /// Parameters: `(payload, stream, created, correlation, causation, tenant_id)`.
    pub insert: String,
}

impl SaaSOutboxQueries {
    pub fn new(naming: &PGNaming, payload_type: &str) -> Self {
        let t = naming.table("outbox");
        Self {
            setup: ddl::outbox_statements(naming, payload_type),
            insert: format!(
                "insert into {t} (payload, stream, created, correlation, causation, tenant_id) values ($1, $2, $3, $4, $5, $6)"
            ),
        }
    }
}
