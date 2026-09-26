//! PostgreSQL wire types for encoded payloads (requires the `sqlx` feature).
//!
//! Storage drivers hold payloads as bytes produced by a
//! [`Codec`](edomata_backend::Codec) and bind them with one of these
//! wrappers, which put the bytes on the wire in the column's native format:
//!
//! - [`JsonbPayload`]: binary `jsonb` = version byte `1` followed by the
//!   UTF-8 JSON text (what `sqlx::types::Json` does, minus the intermediate
//!   `serde_json::Value`);
//! - [`JsonPayload`]: `json`, the JSON text itself;
//! - [`ByteaPayload`]: `bytea`, opaque bytes.
//!
//! [`PgPayload`] dispatches on a [`PayloadFormat`] at runtime.

use std::error::Error;

use edomata_backend::PayloadFormat;
use sqlx::encode::IsNull;
use sqlx::postgres::{PgArgumentBuffer, PgTypeInfo, PgValueFormat, PgValueRef, Postgres};
use sqlx::{Decode, Encode, Type, TypeInfo, ValueRef};

type BoxDynError = Box<dyn Error + Send + Sync + 'static>;

/// Version byte of the binary `jsonb` wire format.
pub const JSONB_VERSION: u8 = 1;

/// JSON bytes stored in a `jsonb` column.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct JsonbPayload(pub Vec<u8>);

/// JSON bytes stored in a `json` column.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct JsonPayload(pub Vec<u8>);

/// Bytes stored in a `bytea` column.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ByteaPayload(pub Vec<u8>);

fn jsonb_type() -> PgTypeInfo {
    PgTypeInfo::with_name("jsonb")
}

fn json_type() -> PgTypeInfo {
    PgTypeInfo::with_name("json")
}

fn is_json_like(ty: &PgTypeInfo) -> bool {
    matches!(ty.name().to_ascii_lowercase().as_str(), "jsonb" | "json")
}

impl Type<Postgres> for JsonbPayload {
    fn type_info() -> PgTypeInfo {
        jsonb_type()
    }

    fn compatible(ty: &PgTypeInfo) -> bool {
        is_json_like(ty)
    }
}

impl Encode<'_, Postgres> for JsonbPayload {
    fn encode_by_ref(&self, buf: &mut PgArgumentBuffer) -> Result<IsNull, BoxDynError> {
        buf.push(JSONB_VERSION);
        buf.extend_from_slice(&self.0);
        Ok(IsNull::No)
    }

    fn size_hint(&self) -> usize {
        self.0.len() + 1
    }
}

impl Decode<'_, Postgres> for JsonbPayload {
    fn decode(value: PgValueRef<'_>) -> Result<Self, BoxDynError> {
        Ok(JsonbPayload(decode_json_like(value)?))
    }
}

impl Type<Postgres> for JsonPayload {
    fn type_info() -> PgTypeInfo {
        json_type()
    }

    fn compatible(ty: &PgTypeInfo) -> bool {
        is_json_like(ty)
    }
}

impl Encode<'_, Postgres> for JsonPayload {
    fn encode_by_ref(&self, buf: &mut PgArgumentBuffer) -> Result<IsNull, BoxDynError> {
        buf.extend_from_slice(&self.0);
        Ok(IsNull::No)
    }

    fn size_hint(&self) -> usize {
        self.0.len()
    }
}

impl Decode<'_, Postgres> for JsonPayload {
    fn decode(value: PgValueRef<'_>) -> Result<Self, BoxDynError> {
        Ok(JsonPayload(decode_json_like(value)?))
    }
}

impl Type<Postgres> for ByteaPayload {
    fn type_info() -> PgTypeInfo {
        <Vec<u8> as Type<Postgres>>::type_info()
    }

    fn compatible(ty: &PgTypeInfo) -> bool {
        <Vec<u8> as Type<Postgres>>::compatible(ty)
    }
}

impl Encode<'_, Postgres> for ByteaPayload {
    fn encode_by_ref(&self, buf: &mut PgArgumentBuffer) -> Result<IsNull, BoxDynError> {
        <Vec<u8> as Encode<'_, Postgres>>::encode_by_ref(&self.0, buf)
    }

    fn size_hint(&self) -> usize {
        self.0.len()
    }
}

impl Decode<'_, Postgres> for ByteaPayload {
    fn decode(value: PgValueRef<'_>) -> Result<Self, BoxDynError> {
        Ok(ByteaPayload(<Vec<u8> as Decode<'_, Postgres>>::decode(
            value,
        )?))
    }
}

/// Decodes a `json` or `jsonb` value to its JSON text bytes, whatever the
/// wire format and column type.
fn decode_json_like(value: PgValueRef<'_>) -> Result<Vec<u8>, BoxDynError> {
    let is_jsonb = value.type_info().name().eq_ignore_ascii_case("jsonb");
    let bytes = value.as_bytes()?;
    match value.format() {
        PgValueFormat::Binary if is_jsonb => match bytes.split_first() {
            Some((&JSONB_VERSION, rest)) => Ok(rest.to_vec()),
            Some((version, _)) => Err(format!("unsupported JSONB format version {version}").into()),
            None => Err("empty JSONB value".into()),
        },
        PgValueFormat::Binary | PgValueFormat::Text => Ok(bytes.to_vec()),
    }
}

/// An encoded payload together with its column type, for drivers that pick
/// the type at runtime from a codec's [`PayloadFormat`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PgPayload {
    /// Stored in a `jsonb` column.
    Jsonb(JsonbPayload),
    /// Stored in a `json` column.
    Json(JsonPayload),
    /// Stored in a `bytea` column.
    Bytea(ByteaPayload),
}

impl PgPayload {
    /// Wraps encoded bytes for the given column type.
    pub fn new(format: PayloadFormat, bytes: Vec<u8>) -> Self {
        match format {
            PayloadFormat::Jsonb => PgPayload::Jsonb(JsonbPayload(bytes)),
            PayloadFormat::Json => PgPayload::Json(JsonPayload(bytes)),
            PayloadFormat::Bytea => PgPayload::Bytea(ByteaPayload(bytes)),
        }
    }

    /// The column type.
    pub fn format(&self) -> PayloadFormat {
        match self {
            PgPayload::Jsonb(_) => PayloadFormat::Jsonb,
            PgPayload::Json(_) => PayloadFormat::Json,
            PgPayload::Bytea(_) => PayloadFormat::Bytea,
        }
    }

    /// The encoded bytes.
    pub fn bytes(&self) -> &[u8] {
        match self {
            PgPayload::Jsonb(p) => &p.0,
            PgPayload::Json(p) => &p.0,
            PgPayload::Bytea(p) => &p.0,
        }
    }

    /// Consumes the payload, returning the encoded bytes.
    pub fn into_bytes(self) -> Vec<u8> {
        match self {
            PgPayload::Jsonb(p) => p.0,
            PgPayload::Json(p) => p.0,
            PgPayload::Bytea(p) => p.0,
        }
    }

    /// Decodes a value of the given column type.
    pub fn decode_as(format: PayloadFormat, value: PgValueRef<'_>) -> Result<Self, BoxDynError> {
        Ok(match format {
            PayloadFormat::Jsonb => PgPayload::Jsonb(JsonbPayload::decode(value)?),
            PayloadFormat::Json => PgPayload::Json(JsonPayload::decode(value)?),
            PayloadFormat::Bytea => PgPayload::Bytea(ByteaPayload::decode(value)?),
        })
    }
}

impl Type<Postgres> for PgPayload {
    fn type_info() -> PgTypeInfo {
        jsonb_type()
    }

    fn compatible(ty: &PgTypeInfo) -> bool {
        is_json_like(ty) || <Vec<u8> as Type<Postgres>>::compatible(ty)
    }
}

impl Encode<'_, Postgres> for PgPayload {
    fn encode_by_ref(&self, buf: &mut PgArgumentBuffer) -> Result<IsNull, BoxDynError> {
        match self {
            PgPayload::Jsonb(p) => p.encode_by_ref(buf),
            PgPayload::Json(p) => p.encode_by_ref(buf),
            PgPayload::Bytea(p) => p.encode_by_ref(buf),
        }
    }

    fn produces(&self) -> Option<PgTypeInfo> {
        Some(match self {
            PgPayload::Jsonb(_) => jsonb_type(),
            PgPayload::Json(_) => json_type(),
            PgPayload::Bytea(_) => <Vec<u8> as Type<Postgres>>::type_info(),
        })
    }

    fn size_hint(&self) -> usize {
        self.bytes().len() + 1
    }
}

impl Decode<'_, Postgres> for PgPayload {
    fn decode(value: PgValueRef<'_>) -> Result<Self, BoxDynError> {
        let name = value.type_info().name().to_ascii_lowercase();
        match name.as_str() {
            "jsonb" => Ok(PgPayload::Jsonb(JsonbPayload::decode(value)?)),
            "json" => Ok(PgPayload::Json(JsonPayload::decode(value)?)),
            "bytea" => Ok(PgPayload::Bytea(ByteaPayload::decode(value)?)),
            other => Err(format!("unsupported payload column type {other}").into()),
        }
    }
}
