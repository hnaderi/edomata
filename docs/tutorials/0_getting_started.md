# Getting started

## Install 

```scala
libraryDependencies += "io.github.hnaderi" %% "edomata-core" % "@VERSION@"
```

or for scala.js
```scala
libraryDependencies += "io.github.hnaderi" %%% "edomata-core" % "@VERSION@"
```

## Imports

```scala mdoc
import edomata.core.*
import edomata.syntax.all.* // for convenient extension methods
```

## Decision
It all start with decisions!  
Decision models programs that decide in a event driven context, these programs are pure and can:  
- accept events  
- reject with errors  
- stay indecisive!  

```scala mdoc
val d1 = Decision(1)
val d2 = Decision.accept("Missile Launched!")
val d3 = Decision.reject("No remained missiles to launch!")
```

decisions are composable  
```scala mdoc
val d4 = d1.map(_ * 2)
val d5 = d2 >> d1
val d6 = d5 >> d3
```

you can also use for comprehensions:  
```scala mdoc
val d7 = for {
  i <- Decision.pure(1)
  _ <- Decision.accept("A") // accepting one event
  _ <- Decision.accept("B", "C") // accepting several events
  j <- Decision.acceptReturn(i * 2)("D", "E") // accepting several events and returning a value
} yield i + j

```

or in presence of cats syntax:  
```scala mdoc
import cats.implicits.*     // for cats related stuff

val d8 = (d1, d7).mapN(_ + _)
val d9 = List.range(1, 3).traverse(Decision.accept(_))
val d10 = Decision(List.range(1, 3)).sequence[List, Int]
```

## DomainModel

TBD

## Edomaton

TBD

## What's next?

You can go to next tutorial or get yourself more familiar with the following.

## More

TBD
