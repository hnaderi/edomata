# Running

A pretty common pattern in functional programming, is creating programs that defines a problem and then creating other programs that interpret them; and thus decoupling what and how of a program. there are a bunch of terms used for such a pattern but I'll stick with program and interpreter here.
an `Edomaton` is simply a definition of an action that needs to take place, it won't affect its environment even if it runs for several times (and programs that uses side effects must follow this rule); it just computes the result of decision making, so we can use it.

For running such a program we need another program that can interpret this one, which in edomata is called a backend.

## Backends

there are currently 2 backends supported, but creating new ones is pretty straight forward as much as extending or customizing provided ones.
currently available backends rely on RDBMSes (specifically Postgres) for persistence.

### Postgres

these backends are designed to be as standard as possible and to be a great fit as default backend in production scenarios for microservice/service based/event-driven architectures.  

#### Persistence
Each aggregate type has its own namespace (which is a separate schema in postgres), that contains all the required tables:

- journal: where events are stored
- outbox: where notifications are stored for consuming
- commands: for idempotency checking of commands
- snapshots: for persisting snapshots, if configured to do so  

Beside that, these backends also have built-in caches that when sized correctly, can prevent most of the database stuff and runs application like a [memory image](https://martinfowler.com/bliki/MemoryImage.html) which can boost a lot of performance. this database stuff are required if you need to ensure no dataloss in a normal webservice/microservice scenario, which is the most common use case; however there is nothing that prevents a backend to use a disruptor pattern (like LMAX disruptor) to eliminates all the disk stuff while running business code.

These 2 backends are completely compatible with each others data, so you can change your mind on which one to use any time.

#### Serialization

both backends support `json`, `jsonb`, `bytea` types in postgres, so you can use the one which is more appropriate.
integrations with `Circe` and `uPickle` is provided in separate modules which you can use, or otherwise you can create your own codecs easily.

#### Usage

for instructions on how to setup and use this backends refer to:

- [Skunk backend](../backends/skunk.md)
- [Doobie backend](../backends/doobie.md)
