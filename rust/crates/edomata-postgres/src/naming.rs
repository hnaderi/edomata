//! Table naming strategies.

use crate::{PGNamespace, PGNamespaceError};

/// How the tables of one aggregate are named.
///
/// - [`PGNaming::Schema`] (the default in Scala) creates a dedicated
///   PostgreSQL schema per aggregate, e.g. `"auth".journal`.
/// - [`PGNaming::Prefixed`] uses a table-name prefix inside the current
///   schema, e.g. `auth_journal`, which suits single-schema projects and
///   Flyway migrations in `public`. Constraint and index names are prefixed
///   too, to avoid collisions between aggregates.
///
/// ```
/// use edomata_postgres::{PGNaming, PGNamespace};
///
/// let ns = PGNamespace::try_from("auth").unwrap();
/// let schema = PGNaming::schema(ns.clone());
/// assert_eq!(schema.table("journal"), "\"auth\".journal");
/// assert_eq!(schema.constraint("journal_pk"), "journal_pk");
/// assert!(schema.needs_schema_setup());
///
/// let prefixed = PGNaming::prefixed(ns);
/// assert_eq!(prefixed.table("journal"), "auth_journal");
/// assert_eq!(prefixed.index("journal_seqnr_idx"), "auth_journal_seqnr_idx");
/// assert!(!prefixed.needs_schema_setup());
/// ```
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum PGNaming {
    /// Schema-based naming: `"namespace".table_name`.
    Schema(PGNamespace),
    /// Prefix-based naming: `namespace_table_name`.
    Prefixed(PGNamespace),
}

impl PGNaming {
    /// Schema-based naming from a validated namespace.
    pub fn schema(ns: PGNamespace) -> Self {
        PGNaming::Schema(ns)
    }

    /// Prefix-based naming from a validated namespace.
    pub fn prefixed(ns: PGNamespace) -> Self {
        PGNaming::Prefixed(ns)
    }

    /// Schema-based naming, validating the namespace.
    pub fn schema_str(ns: &str) -> Result<Self, PGNamespaceError> {
        PGNamespace::from_string(ns).map(PGNaming::Schema)
    }

    /// Prefix-based naming, validating the namespace.
    pub fn prefixed_str(ns: &str) -> Result<Self, PGNamespaceError> {
        PGNamespace::from_string(ns).map(PGNaming::Prefixed)
    }

    /// The underlying namespace.
    pub fn namespace(&self) -> &PGNamespace {
        match self {
            PGNaming::Schema(ns) | PGNaming::Prefixed(ns) => ns,
        }
    }

    /// Whether a `CREATE SCHEMA` statement is needed during setup.
    pub fn needs_schema_setup(&self) -> bool {
        matches!(self, PGNaming::Schema(_))
    }

    /// A fully-qualified (schema mode) or prefixed (prefix mode) table
    /// reference suitable for embedding in SQL.
    pub fn table(&self, name: &str) -> String {
        match self {
            PGNaming::Schema(ns) => format!("\"{ns}\".{name}"),
            PGNaming::Prefixed(ns) => format!("{ns}_{name}"),
        }
    }

    /// A constraint name, prefixed in prefix mode.
    pub fn constraint(&self, name: &str) -> String {
        match self {
            PGNaming::Schema(_) => name.to_owned(),
            PGNaming::Prefixed(ns) => format!("{ns}_{name}"),
        }
    }

    /// An index name, prefixed in prefix mode.
    pub fn index(&self, name: &str) -> String {
        match self {
            PGNaming::Schema(_) => name.to_owned(),
            PGNaming::Prefixed(ns) => format!("{ns}_{name}"),
        }
    }
}
