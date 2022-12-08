# Getting started

## Add to your build

```scala
libraryDependencies += "dev.hnaderi" %% "edomata-core" % "@VERSION@"
```

or for scala.js
```scala
libraryDependencies += "dev.hnaderi" %%% "edomata-core" % "@VERSION@"
```

## Layers of abstraction

Before jumping in to code, we should know the big picture of what we are going to do;

#### Layers

application logic is divided into 2 separate layers:

* domain
    * is pure and has no side effects at all
    * ensures aggregate invariants and implements all business rules and logic
    * does not change frequently in well established businesses
    * models an aggregate root in DDD
    * aggregate roots just hold enough data to decide based on business rules and maintain invariants; they don't care about any other data
* service
    * may involve side effects
    * is very minimal and mostly gluing
    * must be idempotent
    * models application services and command handlers in DDD
  
#### Aggregate space

* each aggregate has a unique address
* all the possible aggregates together, form our aggregate space which is an infinite space (mathematically speaking)
* there is nothing as a non existing aggregate, aggregates that are not created yet do exist in their initial state.

#### Variants

Currently these kinds of state machines are implemented in edomata:

* `Edomaton` which is a portmanteau for event driven automaton, and models applications that are event sourced.
* `Stomaton`, which is also a portmanteau (surprisingly) for state driven automaton, 
and models applications that are simple state machines (without event sourcing); 
and is especially useful in CQRS style command handling.


## Next
That's all we need to know for now; talk is enough, let's jump into code!

for event sourced applications, [follow this link](1-1_eventsourcing.md),  
for cqrs applications, [follow this link](1-2_cqrs.md)
