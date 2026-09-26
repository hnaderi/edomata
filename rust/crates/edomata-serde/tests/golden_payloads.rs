//! Golden payload tests: payloads written by the Scala codecs (Circe,
//! jsoniter-scala, uPickle) must be readable by `SerdeCodec`, and the Rust
//! encoding must be byte-identical when the representation allows it.
//!
//! Files under `rust/tests/golden/payloads/` are produced by
//! `examples/src/test/scala/GoldenPayloads.scala`:
//!
//! ```text
//! sbt "examplesJVM/Test/runMain golden.GoldenPayloads rust/tests/golden/payloads"
//! ```

use std::path::PathBuf;

use edomata_backend::Codec;
use edomata_serde::SerdeCodec;
use edomata_serde::compat::upickle_option;
use serde::{Deserialize, Serialize};

fn golden(name: &str) -> Vec<u8> {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/golden/payloads")
        .join(name);
    std::fs::read(&path).unwrap_or_else(|e| panic!("missing golden file {}: {e}", path.display()))
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
struct Money {
    amount: i64,
    currency: String,
}

const SAMPLES: [&str; 4] = ["opened", "deposited_note", "deposited_no_note", "closed"];

// --- Circe: externally tagged, `{}` for parameterless cases, `null` for None

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
enum CirceEvent {
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
    Closed {},
}

fn circe_expected(name: &str) -> CirceEvent {
    match name {
        "opened" => CirceEvent::Opened {
            owner: "bob".into(),
            initial: Money {
                amount: 100,
                currency: "EUR".into(),
            },
            tags: vec!["vip".into(), "eu".into()],
        },
        "deposited_note" => CirceEvent::Deposited {
            amount: 42,
            note: Some("salary".into()),
            verified: true,
        },
        "deposited_no_note" => CirceEvent::Deposited {
            amount: 7,
            note: None,
            verified: false,
        },
        "closed" => CirceEvent::Closed {},
        other => panic!("unknown sample {other}"),
    }
}

#[test]
fn circe_payloads_are_readable_and_reproduced_byte_for_byte() {
    let codec = SerdeCodec::<CirceEvent>::jsonb();
    for name in SAMPLES {
        let bytes = golden(&format!("circe_{name}.json"));
        let decoded = codec
            .decode(&bytes)
            .unwrap_or_else(|e| panic!("{name}: {e}"));
        assert_eq!(decoded, circe_expected(name), "{name}");
        assert_eq!(
            codec.encode(&decoded).unwrap(),
            bytes,
            "{name}: Rust encoding differs from Circe"
        );
    }
}

// --- jsoniter: internally tagged with "type", None fields omitted

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type")]
enum JsoniterEvent {
    Opened {
        owner: String,
        initial: Money,
        tags: Vec<String>,
    },
    Deposited {
        amount: i64,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        note: Option<String>,
        verified: bool,
    },
    Closed,
}

fn jsoniter_expected(name: &str) -> JsoniterEvent {
    match name {
        "opened" => JsoniterEvent::Opened {
            owner: "bob".into(),
            initial: Money {
                amount: 100,
                currency: "EUR".into(),
            },
            tags: vec!["vip".into(), "eu".into()],
        },
        "deposited_note" => JsoniterEvent::Deposited {
            amount: 42,
            note: Some("salary".into()),
            verified: true,
        },
        "deposited_no_note" => JsoniterEvent::Deposited {
            amount: 7,
            note: None,
            verified: false,
        },
        "closed" => JsoniterEvent::Closed,
        other => panic!("unknown sample {other}"),
    }
}

#[test]
fn jsoniter_payloads_are_readable_and_reproduced_byte_for_byte() {
    let codec = SerdeCodec::<JsoniterEvent>::jsonb();
    for name in SAMPLES {
        let bytes = golden(&format!("jsoniter_{name}.json"));
        let decoded = codec
            .decode(&bytes)
            .unwrap_or_else(|e| panic!("{name}: {e}"));
        assert_eq!(decoded, jsoniter_expected(name), "{name}");
        assert_eq!(
            codec.encode(&decoded).unwrap(),
            bytes,
            "{name}: Rust encoding differs from jsoniter"
        );
    }
}

#[test]
fn jsoniter_msgpack_codec_actually_writes_json_and_is_readable() {
    // `JsoniterCodec.msgpack` uses `writeToArray`, which produces JSON bytes.
    let codec = SerdeCodec::<JsoniterEvent>::bytea();
    for name in SAMPLES {
        let bytes = golden(&format!("jsoniter_msgpack_{name}.bin"));
        assert_eq!(bytes, golden(&format!("jsoniter_{name}.json")), "{name}");
        assert_eq!(
            codec.decode(&bytes).unwrap(),
            jsoniter_expected(name),
            "{name}"
        );
    }
}

// --- uPickle: "$type" tag, bare string for parameterless cases, Option as array

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
enum UpickleEvent {
    Tagged(UpickleTagged),
    Singleton(UpickleSingleton),
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "$type")]
enum UpickleTagged {
    Opened {
        owner: String,
        initial: Money,
        tags: Vec<String>,
    },
    Deposited {
        amount: i64,
        #[serde(with = "upickle_option")]
        note: Option<String>,
        verified: bool,
    },
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
enum UpickleSingleton {
    Closed,
}

fn upickle_expected(name: &str) -> UpickleEvent {
    match name {
        "opened" => UpickleEvent::Tagged(UpickleTagged::Opened {
            owner: "bob".into(),
            initial: Money {
                amount: 100,
                currency: "EUR".into(),
            },
            tags: vec!["vip".into(), "eu".into()],
        }),
        "deposited_note" => UpickleEvent::Tagged(UpickleTagged::Deposited {
            amount: 42,
            note: Some("salary".into()),
            verified: true,
        }),
        "deposited_no_note" => UpickleEvent::Tagged(UpickleTagged::Deposited {
            amount: 7,
            note: None,
            verified: false,
        }),
        "closed" => UpickleEvent::Singleton(UpickleSingleton::Closed),
        other => panic!("unknown sample {other}"),
    }
}

#[test]
fn upickle_json_payloads_are_readable_and_reproduced_byte_for_byte() {
    let codec = SerdeCodec::<UpickleEvent>::jsonb();
    for name in SAMPLES {
        let bytes = golden(&format!("upickle_{name}.json"));
        let decoded = codec
            .decode(&bytes)
            .unwrap_or_else(|e| panic!("{name}: {e}"));
        assert_eq!(decoded, upickle_expected(name), "{name}");
        assert_eq!(
            codec.encode(&decoded).unwrap(),
            bytes,
            "{name}: Rust encoding differs from uPickle"
        );
    }
}

#[test]
fn upickle_msgpack_payloads_are_a_documented_limitation() {
    // Real MessagePack in `bytea`: not JSON, so `serde_json` must reject it.
    let codec = SerdeCodec::<UpickleEvent>::bytea();
    for name in SAMPLES {
        let bytes = golden(&format!("upickle_msgpack_{name}.bin"));
        assert!(
            codec.decode(&bytes).is_err(),
            "{name}: MessagePack unexpectedly decoded as JSON"
        );
    }
}

#[test]
fn all_golden_payload_files_are_covered() {
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../tests/golden/payloads");
    let count = std::fs::read_dir(dir)
        .unwrap()
        .filter_map(|e| e.ok())
        .count();
    // 4 samples × (circe, jsoniter, jsoniter msgpack, upickle, upickle msgpack)
    assert_eq!(count, SAMPLES.len() * 5);
}
