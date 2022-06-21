# Doobie

## Install 

```scala
libraryDependencies += "dev.hnaderi" %% "edomata-doobie" % "@VERSION@"
```

or for integrated modules:
```scala
libraryDependencies += "dev.hnaderi" %% "edomata-doobie-circe" % "@VERSION@"
libraryDependencies += "dev.hnaderi" %% "edomata-doobie-upickle" % "@VERSION@"
```

Note that doobie is built on top of JDBC which can't be used in javascript obviously, and this packages are available for JVM only.

## Imports
```scala
import edomata.doobie.*
```

## Defining codecs

```scala
given BackendCodec[Event] = CirceCodec.jsonb // or .json
given BackendCodec[Notification] = CirceCodec.jsonb
```  

### Persisted snapshots
if you want to use persisted snapshots, you need to provide codec for your state model too.
```scala
given BackendCodec[State] = CirceCodec.jsonb 
```  

## Compiling application to a service
```scala
val app  = ??? // your application from previous chapter
val trx : Transactor[IO] = ??? // create your Transactor

val buildBackend = DoobieBackend(pool)
  .builder(AccountService, "domainname")          // 1
  // .persistedSnapshot(maxInMem = 200)  // 2
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

1. use your domain as described in previous chapter. `domainname` is used as schema name in postgres.
2. feel free to navigate available options in backend builder.
