//! Round-trip tests for `SerdeCodec` and the wire wrappers.

use edomata_backend::{Codec, CodecError, PayloadFormat};
use edomata_serde::SerdeCodec;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
struct Money {
    amount: i64,
    currency: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
enum Event {
    Opened {
        owner: String,
        initial: Money,
        tags: Vec<String>,
    },
    Deposited {
        amount: i64,
        note: Option<String>,
        verified: bool,
    },
    Closed,
}

fn samples() -> Vec<Event> {
    vec![
        Event::Opened {
            owner: "bob".into(),
            initial: Money {
                amount: 100,
                currency: "EUR".into(),
            },
            tags: vec!["vip".into(), "eu".into()],
        },
        Event::Deposited {
            amount: 42,
            note: Some("salary".into()),
            verified: true,
        },
        Event::Deposited {
            amount: 7,
            note: None,
            verified: false,
        },
        Event::Closed,
    ]
}

#[test]
fn every_format_round_trips() {
    for codec in [
        SerdeCodec::<Event>::jsonb(),
        SerdeCodec::<Event>::json(),
        SerdeCodec::<Event>::bytea(),
    ] {
        for event in samples() {
            let bytes = codec.encode(&event).unwrap();
            assert_eq!(codec.decode(&bytes).unwrap(), event, "{codec:?}");
        }
    }
}

#[test]
fn bytes_are_compact_json_whatever_the_format() {
    let event = Event::Closed;
    let jsonb = SerdeCodec::<Event>::jsonb().encode(&event).unwrap();
    let json = SerdeCodec::<Event>::json().encode(&event).unwrap();
    let bytea = SerdeCodec::<Event>::bytea().encode(&event).unwrap();
    assert_eq!(jsonb, br#""Closed""#);
    assert_eq!(jsonb, json);
    assert_eq!(jsonb, bytea);
}

#[test]
fn default_is_jsonb() {
    let codec = SerdeCodec::<Event>::default();
    assert_eq!(codec.format(), PayloadFormat::Jsonb);
    assert_eq!(SerdeCodec::<Event>::json().format(), PayloadFormat::Json);
    assert_eq!(SerdeCodec::<Event>::bytea().format(), PayloadFormat::Bytea);
    assert_eq!(
        SerdeCodec::<Event>::with_format(PayloadFormat::Json).format(),
        PayloadFormat::Json
    );
    assert_eq!(PayloadFormat::default(), PayloadFormat::Jsonb);
}

#[test]
fn decoding_garbage_is_a_decode_error() {
    let err = SerdeCodec::<Event>::jsonb()
        .decode(b"not json")
        .unwrap_err();
    assert!(matches!(err, CodecError::Decode(_)));
    assert!(err.to_string().starts_with("decoding failed: "));
}

#[test]
fn codec_is_usable_through_arc_and_box() {
    use std::sync::Arc;
    let arc: Arc<dyn Codec<Event>> = Arc::new(SerdeCodec::jsonb());
    let boxed: Box<dyn Codec<Event>> = Box::new(SerdeCodec::json());
    let event = Event::Closed;
    assert_eq!(arc.decode(&arc.encode(&event).unwrap()).unwrap(), event);
    assert_eq!(boxed.decode(&boxed.encode(&event).unwrap()).unwrap(), event);
    assert_eq!(boxed.format(), PayloadFormat::Json);
}

#[cfg(feature = "sqlx")]
mod wire {
    use edomata_serde::PayloadFormat;
    use edomata_serde::pg::{ByteaPayload, JSONB_VERSION, JsonPayload, JsonbPayload, PgPayload};
    use sqlx::Encode;
    use sqlx::postgres::{PgArgumentBuffer, Postgres};

    fn encode<'q, T: Encode<'q, Postgres>>(value: &T) -> Vec<u8> {
        let mut buf = PgArgumentBuffer::default();
        let _is_null = value.encode_by_ref(&mut buf).unwrap();
        buf.to_vec()
    }

    #[test]
    fn jsonb_wire_format_is_version_byte_plus_json() {
        let bytes = encode(&JsonbPayload(br#"{"a":1}"#.to_vec()));
        assert_eq!(bytes[0], JSONB_VERSION);
        assert_eq!(&bytes[1..], br#"{"a":1}"#);
    }

    #[test]
    fn json_and_bytea_wire_formats_are_the_raw_bytes() {
        assert_eq!(encode(&JsonPayload(br#"{"a":1}"#.to_vec())), br#"{"a":1}"#);
        assert_eq!(encode(&ByteaPayload(vec![1, 2, 3])), vec![1, 2, 3]);
    }

    #[test]
    fn pg_payload_dispatches_on_format() {
        let p = PgPayload::new(PayloadFormat::Jsonb, b"{}".to_vec());
        assert_eq!(p.format(), PayloadFormat::Jsonb);
        assert_eq!(p.bytes(), b"{}");
        assert_eq!(encode(&p), [&[JSONB_VERSION][..], b"{}"].concat());
        let j = PgPayload::new(PayloadFormat::Json, b"{}".to_vec());
        assert_eq!(encode(&j), b"{}");
        let b = PgPayload::new(PayloadFormat::Bytea, vec![9]);
        assert_eq!(b.clone().into_bytes(), vec![9]);
        assert_eq!(encode(&b), vec![9]);
    }
}
