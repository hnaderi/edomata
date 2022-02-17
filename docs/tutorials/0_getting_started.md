# Getting started

To install 

```scala
libraryDependencies += "ir.hnaderi" %% "edomata-core" % "@VERSION@"
```

```scala mdoc
import edomata.core.*
val d1 = Decision.pure(1)
```


```scala mdoc:plantuml
hide empty description
[*] --> State1
State1 --> [*]
State1 : this is a string
State1 : this is another string
State1 -> State2
State2 --> [*]
```

```scala mdoc:plantuml

participant Bob
actor Alice
 
Bob -> Alice : hello
Alice -> Bob : Is it ok?

```
