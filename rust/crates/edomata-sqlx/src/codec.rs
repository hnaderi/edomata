//! Codec adapter used by the driver.

use std::sync::Arc;

use edomata_backend::{BackendError, Codec, PayloadFormat};
use edomata_serde::SerdeCodec;
use edomata_serde::pg::PgPayload;
use serde::Serialize;
use serde::de::DeserializeOwned;
use sqlx::Row;
use sqlx::postgres::PgRow;

/// The codec value the sqlx drivers require for a payload of type `T`: any
/// [`Codec<T>`] behind an `Arc`.
///
/// `Default` is `SerdeCodec::jsonb()` for serde types, which is what
/// `BackendBuilder::build_default` uses.
///
/// ```
/// use edomata_backend::PayloadFormat;
/// use edomata_sqlx::SqlxCodec;
///
/// let default = SqlxCodec::<i32>::default();
/// assert_eq!(default.format(), PayloadFormat::Jsonb);
/// assert_eq!(SqlxCodec::<i32>::json().format(), PayloadFormat::Json);
/// ```
pub struct SqlxCodec<T> {
    inner: Arc<dyn Codec<T>>,
}

impl<T> Clone for SqlxCodec<T> {
    fn clone(&self) -> Self {
        Self {
            inner: Arc::clone(&self.inner),
        }
    }
}

impl<T> std::fmt::Debug for SqlxCodec<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SqlxCodec")
            .field("format", &self.inner.format())
            .finish()
    }
}

impl<T> SqlxCodec<T> {
    /// Wraps any codec.
    pub fn new(codec: impl Codec<T> + 'static) -> Self {
        Self {
            inner: Arc::new(codec),
        }
    }

    /// Wraps a shared codec.
    pub fn from_arc(codec: Arc<dyn Codec<T>>) -> Self {
        Self { inner: codec }
    }

    /// The column type of the payloads.
    pub fn format(&self) -> PayloadFormat {
        self.inner.format()
    }

    /// The SQL type name of the payload column.
    pub fn sql_type(&self) -> &'static str {
        self.format().sql_type()
    }

    /// Encodes a value into a bindable PostgreSQL payload.
    pub fn encode(&self, value: &T) -> Result<PgPayload, BackendError> {
        let bytes = self.inner.encode(value)?;
        Ok(PgPayload::new(self.format(), bytes))
    }

    /// Decodes the payload column `column` of a row.
    pub fn decode_row(&self, row: &PgRow, column: &str) -> Result<T, BackendError> {
        let raw = row
            .try_get_raw(column)
            .map_err(|e| BackendError::persistence(format!("missing column {column}: {e}")))?;
        let payload = PgPayload::decode_as(self.format(), raw)
            .map_err(|e| BackendError::persistence(format!("cannot read column {column}: {e}")))?;
        Ok(self.inner.decode(payload.bytes())?)
    }
}

impl<T> SqlxCodec<T>
where
    T: Serialize + DeserializeOwned + Send + Sync + 'static,
{
    /// A `serde_json` codec for a `jsonb` column.
    pub fn jsonb() -> Self {
        Self::new(SerdeCodec::<T>::jsonb())
    }

    /// A `serde_json` codec for a `json` column.
    pub fn json() -> Self {
        Self::new(SerdeCodec::<T>::json())
    }

    /// A `serde_json` codec for a `bytea` column.
    pub fn bytea() -> Self {
        Self::new(SerdeCodec::<T>::bytea())
    }
}

impl<T> Default for SqlxCodec<T>
where
    T: Serialize + DeserializeOwned + Send + Sync + 'static,
{
    fn default() -> Self {
        Self::jsonb()
    }
}
