//! Validated PostgreSQL identifiers.

use std::fmt;
use std::ops::Deref;
use std::str::FromStr;

/// Maximum length of a PostgreSQL identifier.
pub const MAX_LEN: usize = 63;

const PATTERN: &str = "([A-Za-z_][A-Za-z_0-9$]*)";

/// A validated PostgreSQL identifier used to name a schema or to prefix
/// tables. Mirrors Scala's `PGNamespace` opaque type: letters, digits,
/// `_` and `$`, not starting with a digit or `$`, at most 63 characters.
///
/// ```
/// use edomata_postgres::PGNamespace;
///
/// let ns = PGNamespace::try_from("auth").unwrap();
/// assert_eq!(ns.as_str(), "auth");
/// assert!(PGNamespace::try_from("").is_err());
/// assert!(PGNamespace::try_from("1abc").is_err());
/// assert!(PGNamespace::try_from("a".repeat(64).as_str()).is_err());
/// ```
#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct PGNamespace(String);

/// Why a string is not a valid [`PGNamespace`].
#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
pub enum PGNamespaceError {
    /// The name exceeds [`MAX_LEN`] characters.
    #[error("Name is too long: {0} (max allowed is {MAX_LEN})")]
    TooLong(usize),
    /// The name is not a plain identifier.
    #[error("Name: \"{0}\" does not match {PATTERN}")]
    Invalid(String),
}

impl PGNamespace {
    /// Validates `s`, with the same rules and messages as Scala's
    /// `PGNamespace.fromString`.
    pub fn from_string(s: &str) -> Result<Self, PGNamespaceError> {
        let mut chars = s.chars();
        let valid_head = chars
            .next()
            .is_some_and(|c| c.is_ascii_alphabetic() || c == '_');
        let valid_tail = chars.all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '$');
        if !(valid_head && valid_tail) {
            return Err(PGNamespaceError::Invalid(s.to_owned()));
        }
        let len = s.chars().count();
        if len > MAX_LEN {
            return Err(PGNamespaceError::TooLong(len));
        }
        Ok(Self(s.to_owned()))
    }

    /// The identifier as a string slice.
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// Prefix-based naming for this namespace (Scala's `PGNamespace.prefixed`).
    pub fn prefixed(self) -> crate::PGNaming {
        crate::PGNaming::Prefixed(self)
    }

    /// Schema-based naming for this namespace.
    pub fn schema(self) -> crate::PGNaming {
        crate::PGNaming::Schema(self)
    }
}

impl TryFrom<&str> for PGNamespace {
    type Error = PGNamespaceError;

    fn try_from(s: &str) -> Result<Self, PGNamespaceError> {
        Self::from_string(s)
    }
}

impl TryFrom<String> for PGNamespace {
    type Error = PGNamespaceError;

    fn try_from(s: String) -> Result<Self, PGNamespaceError> {
        Self::from_string(&s)
    }
}

impl FromStr for PGNamespace {
    type Err = PGNamespaceError;

    fn from_str(s: &str) -> Result<Self, PGNamespaceError> {
        Self::from_string(s)
    }
}

impl Deref for PGNamespace {
    type Target = str;

    fn deref(&self) -> &str {
        &self.0
    }
}

impl AsRef<str> for PGNamespace {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for PGNamespace {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl From<PGNamespace> for String {
    fn from(ns: PGNamespace) -> String {
        ns.0
    }
}
