# Skunk

## Install 

```scala
libraryDependencies += "dev.hnaderi" %% "edomata-skunk" % "@VERSION@"
```

or for integrated modules:
```scala
libraryDependencies += "dev.hnaderi" %% "edomata-skunk-circe" % "@VERSION@"
libraryDependencies += "dev.hnaderi" %% "edomata-skunk-upickle" % "@VERSION@"
```

or for scala.js
```scala
libraryDependencies += "dev.hnaderi" %%% "edomata-skunk" % "@VERSION@"
```

## Imports
```scala
import edomata.skunk.*
```

## Defining codecs

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

You need to use a driver to build your backend, there are two skunk drivers available:

1. `SkunkDriver` event sourcing driver  
2. `SkunkCQRSDriver` cqrs driver

```scala
val app  = ??? // your application from previous chapter
val pool : Resource[IO, Session[IO]] = ??? // create your own session pool

val buildBackend = Backend
  .builder(AccountService)               // 1
  .use(SkunkDriver("domainname", pool))  // 2
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
