# Processes

## Reading

```scala mdoc:invisible
import cats.implicits.*
import cats.effect.IO
import edomata.backend.*
import fs2.Stream
import fs2.Stream.*

// Types from previous chapter
trait Account
trait Event
trait Notification
trait Rejection
```

Having a backend at hand from previous chapters like:
```scala mdoc
def backend : Backend[IO, Account, Event, Rejection, Notification] = ???
```

### Outbox
For consuming outboxed items, you can use `backend.outbox` directly, or you can use the provided `OutboxConsumer` like the following:
```scala mdoc
def publisher : Stream[IO, Nothing] = OutboxConsumer(backend){ item =>
  // use outboxed item
  ???
}
```

`OutboxConsumer` will consume outboxed items, run the provided action on each of them, marks all items as read.  
It does not handle or recover action failures to simplify error handling on application side.

> Note that outbox pattern is meant for atomically publishing external messages, and while it has useful data in it, it is not meant to be the medium for processing messages like a queue; and it's best practice to have exactly one consumer on each outbox

### Journal

for reading all journal from the beginning you can use:
```scala mdoc
def all = backend.journal.readAll
```

If you are only interested in a single stream, you can use
```scala mdoc
def singleStream = backend.journal.readStream("interesting-stream")
```

you can also read before or after a specific sequence number
```scala mdoc
def allAfter = backend.journal.readAllAfter(100)
def singleBefore = backend.journal.readStreamBefore("interesting-stream" ,100)
```

### Repository

You can read the entire history of a single aggregate root
```scala mdoc
def history : Stream[IO, AggregateState[Account, Event, Rejection]] = backend.repository.history("interesting-stream")
```

#### Tip
> Note that when you read from repository, you get `AggregateState`, which contains aggregate version on valid streams, or last valid state along with errors and the event that caused this conflict.  
> It's worth noting that your journal are never gonna become corrupt by conflicting decisions, as they are prevented before being written to the journal by conflict protection implemented in Edomata; however if you change 
> the meaning of events, you can change the meaning of history, and face a conflicting stream history.
> It is one of the most important assumptions of event sourcing, that history won't change its meaning and is not specific to this library, so you should invest in more compatibility testing when you are doing a migration or huge change.

Or read current state of the write side projection
```scala mdoc
def current : IO[AggregateState[Account, Event, Rejection]] = backend.repository.get("interesting-stream") 
```


## Integrating
You can easily use provided streams and read functionality to implement any complex processes or business workflows.  
for example a process manager would be just a possibly stateful stream that handles notifications, 
or a new custom read side projections would be just another stream that handle journal events.
