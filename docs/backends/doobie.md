---
sidebar_position: 2
title: "Doobie"
---

# Doobie

## Install 

```scala
libraryDependencies += "io.github.beyond-scale-group" %% "edomata-doobie" % "@VERSION@"
```

or for integrated modules:
```scala
libraryDependencies += "io.github.beyond-scale-group" %% "edomata-doobie-circe" % "@VERSION@"
libraryDependencies += "io.github.beyond-scale-group" %% "edomata-doobie-upickle" % "@VERSION@"
```

Note that doobie is built on top of JDBC which can't be used in javascript obviously, and this packages are available for JVM only.

## Imports
```scala
import edomata.doobie.*
```

## Defining codecs

when event sourcing:
```scala
given BackendCodec[Event] = CirceCodec.jsonb // or .json
given BackendCodec[Notification] = CirceCodec.jsonb
```  

when using cqrs style:
```scala
given BackendCodec[State] = CirceCodec.jsonb 
```  

### Persisted snapshots (event sourcing only)
if you want to use persisted snapshots, you need to provide codec for your state model too.
```scala
given BackendCodec[State] = CirceCodec.jsonb 
```  

## Compiling application to a service

You need to use a driver to build your backend, there are two doobie drivers available:

1. `DoobieDriver` event sourcing driver  
2. `DoobieCQRSDriver` cqrs driver

```scala
val app  = ??? // your application from previous chapter
val trx : Transactor[IO] = ??? // create your Transactor

val buildBackend = Backend
  .builder(AccountService)               // 1
  .use(DoobieDriver("domainname", trx))  // 2
  // .persistedSnapshot(maxInMem = 200)  // 3
  .inMemSnapshot(200)
  .withRetryConfig(retryInitialDelay = 2.seconds)
  .build

val application = buildBackend.use { backend =>
  val service = backend.compile(app)
  // compiling your application will give you a function
  // that takes a messages and does everything required,
  // and returns result.

  service(
    CommandMessage("abc", Instant.now, "a", "receive")
  ).flatMap(IO.println)
}
```

1. use your domain as described in previous chapter. 
2. `domainname` is used as schema name in postgres.
3. feel free to navigate available options in backend builder.

## Table prefix mode

By default, each aggregate gets its own PostgreSQL schema (e.g. `"domainname".journal`).
If you prefer to keep all tables in a single schema (e.g. when using Flyway migrations), you can use prefix-based naming instead:

```scala
import edomata.backend.PGNamespace

val buildBackend = Backend
  .builder(AccountService)
  .use(DoobieDriver.from(PGNamespace.prefixed("domainname"), trx))
  .inMemSnapshot(200)
  .build
```

This creates tables named `domainname_journal`, `domainname_outbox`, etc. in the current schema, and skips the `CREATE SCHEMA` statement.

You can also use `PGNaming` directly for more control:

```scala
import edomata.backend.PGNaming

// Schema mode (default behavior)
DoobieDriver.from(PGNaming.schema("domainname"), trx)

// Prefix mode
DoobieDriver.from(PGNaming.prefixed("domainname"), trx)
```

## Using with Flyway

If you manage your database schema with Flyway (or another migration tool), you can extract the DDL and disable automatic table creation:

### 1. Generate migration SQL

Use `PGSchema` to generate DDL statements for your migration files:

```scala
import edomata.backend.{PGNaming, PGSchema}

// For event sourcing
val ddl = PGSchema.eventsourcing(
  PGNaming.prefixed("accounts"),
  eventType = "jsonb",
  notificationType = "jsonb",
  snapshotType = "jsonb"
)
ddl.foreach(println)

// For CQRS
val cqrsDdl = PGSchema.cqrs(
  PGNaming.prefixed("accounts"),
  stateType = "jsonb",
  notificationType = "jsonb"
)
```

Copy the output into a Flyway migration file (e.g. `V1__create_accounts_tables.sql`).

### 2. Disable automatic setup

Pass `skipSetup = true` to prevent the driver from executing any DDL:

```scala
val buildBackend = Backend
  .builder(AccountService)
  .use(DoobieDriver.from(PGNaming.prefixed("accounts"), trx, skipSetup = true))
  .inMemSnapshot(200)
  .build
```

This ensures Flyway has full control over your database schema.
