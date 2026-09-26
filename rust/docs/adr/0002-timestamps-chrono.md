# ADR 0002: Timestamps use `chrono`

- Status: accepted
- Date: 2026-09-25
- Milestone: 1 (workspace + core)

## Context

Scala uses `java.time.Instant` for `CommandMessage.time` and
`OffsetDateTime` in the PostgreSQL layers. The plan allows either `chrono`
or `time` and asks for the choice to be recorded.

## Decision

Use `chrono` with `DateTime<Utc>` everywhere a timestamp appears:
`CommandMessage::time`, event metadata, outbox items and snapshots.

`edomata-core` depends on `chrono` with `default-features = false` and only
the `std` feature, so no clock or OS dependency is pulled in and the crate
still builds for `wasm32-unknown-unknown`. The `serde` feature of core
enables `chrono/serde`.

## Rationale

- `sqlx` supports both crates, so wire compatibility with the Scala tables
  (`timestamptz`) is identical either way.
- `chrono` is the more widely used crate in the ecosystem the port targets
  (`sqlx`, `rdkafka`, `lapin` users), which minimises conversions at the
  boundaries.
- `DateTime<Utc>` serialises to RFC 3339 by default, matching how Circe and
  jsoniter render `Instant` in JSON payloads.

## Consequences

- Users who prefer `time` convert at the boundary; no `time` API is exposed.
- `Utc` is fixed: Scala's `Instant` has no offset either, so nothing is lost.
