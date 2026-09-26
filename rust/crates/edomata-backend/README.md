# edomata-backend

Backend abstractions for event sourcing and CQRS: repositories, journal and outbox readers, snapshot stores, command handling with retries, caches, update signals, the `Codec` trait, and an in-memory driver reproducing the PostgreSQL constraints.

- **Scala module(s)**: `backend`
- **Book chapter**: [Running](../../book/src/tutorials/backends.md)
- **API documentation**: `cargo doc -p edomata-backend --no-deps --open`

## Main items

- `eventsourcing::{Backend, StorageDriver, Repository, JournalReader, SnapshotStore, CommandHandler}`
- `cqrs::{Backend, StorageDriver, Repository, CommandHandler}`
- `OutboxReader`, `OutboxItem`, `OutboxConsumer`, `CommandStore`, `LruCache`, `Signal`, `RetryConfig`
- `Codec`, `PayloadFormat`, `BackendError`
- `inmemory::{InMemoryDriver, InMemoryEventStore, InMemoryStateStore}`

Part of the [Edomata Rust port](../../README.md); see [`PORTING.md`](../../PORTING.md) for the mapping to the Scala library and [`docs/adr/`](../../docs/adr/) for the design decisions.
