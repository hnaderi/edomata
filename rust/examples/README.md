# edomata-examples

Runnable ports of the Scala programs under `examples/src/main/scala/`, one binary each, plus the broker relays. All of them use the PostgreSQL instance of the repository's `docker-compose.yml` (or `DATABASE_URL`).

| Binary | Scala original | Run |
|--------|----------------|-----|
| `counter` | `Example1.scala` | `cargo run -p edomata-examples --bin counter` |
| `stomaton` | `StomatonExample.scala` | `cargo run -p edomata-examples --bin stomaton` |
| `migration` | `MigrationExample.scala` | `cargo run -p edomata-examples --bin migration` |
| `saas_todo` | `SaaSExample.scala` | `cargo run -p edomata-examples --bin saas_todo` |
| `product_catalog` | `ProductCatalogExample.scala` | `cargo run -p edomata-examples --bin product_catalog` |
| `kafka_relay` | *(new)* | `KAFKA_BOOTSTRAP=localhost:9092 cargo run -p edomata-examples --features kafka --bin kafka_relay` |
| `rabbitmq_relay` | *(new)* | `AMQP_URL=amqp://guest:guest@localhost:5672/%2f cargo run -p edomata-examples --features rabbitmq --bin rabbitmq_relay` |

The `kafka` / `rabbitmq` features are the only way a broker client enters the build. See the [book](../book/src/SUMMARY.md) for the chapters these examples illustrate.
