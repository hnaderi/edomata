//! Helpers for reading payloads written by the Scala codecs.

/// uPickle (3.x, default configuration) writes `Option[T]` as a 0- or
/// 1-element JSON array (`[]` / `["x"]`). Use this module with
/// `#[serde(with = "edomata_serde::compat::upickle_option")]` on
/// `Option<T>` fields of types migrated from uPickle.
///
/// ```
/// use serde::{Deserialize, Serialize};
///
/// #[derive(Debug, PartialEq, Serialize, Deserialize)]
/// struct Deposited {
///     amount: i64,
///     #[serde(with = "edomata_serde::compat::upickle_option")]
///     note: Option<String>,
/// }
///
/// let d: Deposited = serde_json::from_str(r#"{"amount":42,"note":["salary"]}"#).unwrap();
/// assert_eq!(d.note.as_deref(), Some("salary"));
/// let none: Deposited = serde_json::from_str(r#"{"amount":7,"note":[]}"#).unwrap();
/// assert_eq!(none.note, None);
/// assert_eq!(serde_json::to_string(&d).unwrap(), r#"{"amount":42,"note":["salary"]}"#);
/// ```
pub mod upickle_option {
    use serde::de::{Deserialize, Deserializer, Error};
    use serde::ser::{Serialize, SerializeSeq, Serializer};

    /// Serializes `Some(x)` as `[x]` and `None` as `[]`.
    pub fn serialize<T, S>(value: &Option<T>, serializer: S) -> Result<S::Ok, S::Error>
    where
        T: Serialize,
        S: Serializer,
    {
        let mut seq = serializer.serialize_seq(Some(usize::from(value.is_some())))?;
        if let Some(v) = value {
            seq.serialize_element(v)?;
        }
        seq.end()
    }

    /// Deserializes `[]` as `None` and `[x]` as `Some(x)`.
    pub fn deserialize<'de, T, D>(deserializer: D) -> Result<Option<T>, D::Error>
    where
        T: Deserialize<'de>,
        D: Deserializer<'de>,
    {
        let mut items = Vec::<T>::deserialize(deserializer)?;
        match items.len() {
            0 => Ok(None),
            1 => Ok(items.pop()),
            n => Err(D::Error::custom(format!(
                "expected a uPickle Option array of 0 or 1 element, got {n}"
            ))),
        }
    }
}
