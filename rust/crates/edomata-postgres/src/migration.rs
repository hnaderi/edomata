//! Event payload migrations.

use std::fmt;
use std::sync::Arc;

type RunFn = dyn Fn(&str) -> Result<String, String> + Send + Sync;

/// Transforms every journaled event payload from an old format to a new
/// one, like Flyway for event payloads.
///
/// Each migration has a unique version string (like `"001"` or
/// `"v2-add-email"`) and a description. Migrations are applied in list order
/// by the drivers' migration runners; already-applied versions (tracked in
/// the `migrations` table) are skipped, so running them is idempotent.
///
/// Use [`EventMigration::typed`] to decode, transform and re-encode with
/// typed functions; with an exhaustive `match` on an old-event `enum`, the
/// compiler rejects incomplete migrations.
///
/// ```
/// use edomata_postgres::EventMigration;
///
/// let add_suffix = EventMigration::new("001", "Add suffix", |raw| Ok(format!("{raw}!")));
/// let upper = EventMigration::new("002", "Upper case", |raw| Ok(raw.to_uppercase()));
/// let both = add_suffix.and_then(upper);
/// assert_eq!(both.version, "002");
/// assert_eq!(both.run("hi"), Ok("HI!".to_string()));
/// ```
#[derive(Clone)]
pub struct EventMigration {
    /// Unique identifier of this migration.
    pub version: String,
    /// Human-readable description.
    pub description: String,
    run: Arc<RunFn>,
}

impl fmt::Debug for EventMigration {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("EventMigration")
            .field("version", &self.version)
            .field("description", &self.description)
            .finish_non_exhaustive()
    }
}

impl EventMigration {
    /// A migration over raw (JSON text) payloads.
    pub fn new<F>(version: impl Into<String>, description: impl Into<String>, run: F) -> Self
    where
        F: Fn(&str) -> Result<String, String> + Send + Sync + 'static,
    {
        Self {
            version: version.into(),
            description: description.into(),
            run: Arc::new(run),
        }
    }

    /// A typed migration: decode the old payload, transform it, encode the
    /// new one.
    pub fn typed<A, B, D, T, E>(
        version: impl Into<String>,
        description: impl Into<String>,
        decode: D,
        transform: T,
        encode: E,
    ) -> Self
    where
        D: Fn(&str) -> Result<A, String> + Send + Sync + 'static,
        T: Fn(A) -> B + Send + Sync + 'static,
        E: Fn(&B) -> String + Send + Sync + 'static,
    {
        Self::new(version, description, move |raw| {
            decode(raw).map(|a| encode(&transform(a)))
        })
    }

    /// Transforms one raw payload.
    pub fn run(&self, raw: &str) -> Result<String, String> {
        (self.run)(raw)
    }

    /// Composes this migration with a subsequent one. The result has the
    /// version and description of `next`.
    pub fn and_then(self, next: EventMigration) -> EventMigration {
        let first = self.run;
        let second = next.run;
        EventMigration {
            version: next.version,
            description: next.description,
            run: Arc::new(move |raw| first(raw).and_then(|s| second(&s))),
        }
    }
}

/// Outcome of running a list of migrations.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct MigrationResult {
    /// Versions applied by this run, in order.
    pub applied: Vec<String>,
    /// Versions that were already applied and were skipped.
    pub skipped: Vec<String>,
}
