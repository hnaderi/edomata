//! Codec adapter carrying the tenant extractor.

use std::sync::Arc;

use edomata_backend::{BackendError, Codec, PayloadFormat};
use edomata_saas::{TenantExtractor, TenantId, UserId};
use edomata_serde::SerdeCodec;
use edomata_serde::pg::PgPayload;
use edomata_sqlx::SqlxCodec;
use serde::Serialize;
use serde::de::DeserializeOwned;
use sqlx::postgres::PgRow;

type Extractor<T> = dyn Fn(&T) -> Option<(TenantId, UserId)> + Send + Sync;

/// The codec value the SaaS driver requires: a payload codec plus, for
/// states, how to extract the tenant and owner written to the `tenant_id`
/// / `owner_id` columns (Scala's `TenantExtractor` given).
///
/// Use [`SaaSCodec::state`] for the state codec and
/// [`SaaSCodec::notification`] for the notification codec;
/// [`SaaSCodec::jsonb_state`] / [`SaaSCodec::jsonb_notification`] are the
/// serde `jsonb` defaults.
pub struct SaaSCodec<T> {
    codec: SqlxCodec<T>,
    extractor: Option<Arc<Extractor<T>>>,
}

impl<T> Clone for SaaSCodec<T> {
    fn clone(&self) -> Self {
        Self {
            codec: self.codec.clone(),
            extractor: self.extractor.clone(),
        }
    }
}

impl<T> std::fmt::Debug for SaaSCodec<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SaaSCodec")
            .field("format", &self.codec.format())
            .field("tenant_aware", &self.extractor.is_some())
            .finish()
    }
}

impl<T: 'static> SaaSCodec<T> {
    /// A state codec: tenant and owner come from [`TenantExtractor`].
    pub fn state(codec: impl Codec<T> + 'static) -> Self
    where
        T: TenantExtractor,
    {
        Self::with_extractor(codec, TenantExtractor::tenant_and_owner)
    }

    /// A state codec with a custom extractor.
    pub fn with_extractor<F>(codec: impl Codec<T> + 'static, extractor: F) -> Self
    where
        F: Fn(&T) -> Option<(TenantId, UserId)> + Send + Sync + 'static,
    {
        Self {
            codec: SqlxCodec::new(codec),
            extractor: Some(Arc::new(extractor)),
        }
    }

    /// A notification codec (no tenant extraction).
    pub fn notification(codec: impl Codec<T> + 'static) -> Self {
        Self {
            codec: SqlxCodec::new(codec),
            extractor: None,
        }
    }

    /// The underlying sqlx codec.
    pub fn codec(&self) -> &SqlxCodec<T> {
        &self.codec
    }

    /// The column type.
    pub fn format(&self) -> PayloadFormat {
        self.codec.format()
    }

    /// The SQL type name of the payload column.
    pub fn sql_type(&self) -> &'static str {
        self.codec.sql_type()
    }

    /// Encodes a value into a bindable payload.
    pub fn encode(&self, value: &T) -> Result<PgPayload, BackendError> {
        self.codec.encode(value)
    }

    /// Decodes a payload column.
    pub fn decode_row(&self, row: &PgRow, column: &str) -> Result<T, BackendError> {
        self.codec.decode_row(row, column)
    }

    /// The tenant and owner of a value, when this codec is tenant-aware.
    pub fn tenant_and_owner(&self, value: &T) -> Option<(TenantId, UserId)> {
        self.extractor.as_ref().and_then(|f| f(value))
    }
}

impl<T> SaaSCodec<T>
where
    T: Serialize + DeserializeOwned + Send + Sync + 'static,
{
    /// A serde `jsonb` state codec.
    pub fn jsonb_state() -> Self
    where
        T: TenantExtractor,
    {
        Self::state(SerdeCodec::<T>::jsonb())
    }

    /// A serde `jsonb` notification codec.
    pub fn jsonb_notification() -> Self {
        Self::notification(SerdeCodec::<T>::jsonb())
    }
}
