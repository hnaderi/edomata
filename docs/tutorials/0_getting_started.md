# Getting started

To install 

```scala
libraryDependencies += "io.github.hnaderi" %% "edomata-core" % "@VERSION@"
```

```scala mdoc
import edomata.core.*
val d1 = Decision.pure(1)
```

```scala mdoc
import edomata.backend.*
import edomata.backend.FSMDefinition.*

final case class State()
final case class Command()
final case class Event()
final case class Notification()
final case class Rejection()

type ExampleDomain = HasState[State] And
  HasCommand[Command] And
  HasInternalEvent[Event] And
  HasExternalEvent[Notification] And
  HasRejection[Rejection]

```

```scala mdoc
import edomata.backend.*
import cats.Monad

def service[F[_]: Monad] = DomainService2[F, ExampleDomain] {
  case Command() => ???
}

def transition: DomainTransition[ExampleDomain] = ???
```
