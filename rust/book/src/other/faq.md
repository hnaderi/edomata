# FAQ

## Should I use it?

Like everything in engineering, it depends. Answer these questions first.

### Are you sure event sourcing / CQRS is the solution to your problem?

This is the most common problem developers face with tools in this space. If you are not sure, or lack experience with these techniques, chances are high that you won't be happy using them for a real project for the first time: they introduce many new challenges and accidental complexity when used in the wrong context. Consider other techniques, which need other tools.

If you know what you are doing, or want to learn without the commitments of a real project, perfect: you will find this library at least interesting.

### Are you and your team comfortable with asynchronous Rust?

The backends are built on Tokio, sqlx and `futures`. The core is plain Rust values and needs no runtime knowledge, but running a service means `async fn`, streams and a runtime. The Rust community has excellent material on all of this.

## Why yet another library?

See the [rationale](../introduction.md#rationale).

## Can I use a different backend?

Absolutely. Backends are interpreters for your programs: implement the `StorageDriver` trait (or write a function that takes an app and returns a future) and you have one. The in-memory driver in `edomata-backend` is a complete example.

## Can Rust and Scala services share a database?

Yes. The tables and DDL are byte-for-byte identical, `jsonb` payloads written by serde are the same JSON the Scala Circe, jsoniter and uPickle codecs write and read (except uPickle's `msgpack`), and a cross-language test exercises exactly this. See the [migration guide](migration-guide.md).

## What is the point of this library? I can create my own data structures.

Exactly, that's how simple it is. While it is not rocket science to write a monadic data type, it is not what you want to focus on when writing business logic; and building an effect stack out of readers, writers and error channels won't be appealing either. Too much flexibility is cognitive load better spent on real business problems. Edomata is a few opinionated, tailored data types for creating programs, plus production-ready interpreters for them.

## Where are the Kafka / RabbitMQ features?

In `edomata-broker`, `edomata-kafka` and `edomata-rabbitmq`, see [Distributing events](../backends/brokers.md). They are opt-in: nothing in the core or the PostgreSQL backend depends on a broker client.
