# Definitions
following is a list of useful definitions in the context of DDD/CQRS/ES.  
I tried to keep them as generic as possible, but some of them are also mapped to this specific library to help better understanding.

## Message
A packet of data, that can carry some information
```scala mdoc:plantuml:mindmap
* message
** event
** command
** document
```

### Command message
A message that its intent is to convey a request to change system behavior  
commands are named using imperative verbs, like `ReceivePayment`, `ConfirmOrder`, `MarkAsDelivered`.  
command may be rejected by system, according to its logic, policies, state, ...

### Event message
A message that its intent is to capture a fact.  
events are always named using past-participle verbs, like `OrderReceived`, `AssetSecured`, `PaymentReceived`.  
events may not be rejected as they are facts about what happened and you can't deny them.  

### Document message
A message that its purpose is to transfer information, without any other intent, consumers decide on how to act and what is the intent.  
documents are named using nouns, like `HealthReport`, `InvestorWeeklyReport`, `ShippingStatistics`, `CandleData`.  
Most of the time, documents represent aggregated data and are not very thin, and might be even files with a few dozen megabytes in size. (but do not send documents of this size to message brokers please!)



## Components

### Projection
An aggregated data structure per event stream, that maintains a specific view point from historical events.

### Query
A request to ask system for some data. queries are idempotent by nature, and cannot change any system behavior. they can be cached most of the time.

### Event stream
History of events from beginning in chronological order.

### Command handler
A component that receive command message, decides what to do based on domain logic, and acts upon it.  
it is basically a function in the form of `cmd => F[EitherNec[Rejection, Unit]]`.

### Event handler
A component that receives a sub-set of events from one or more event streams, and runs side effects.  
they are basically modeled as a function that transforms an event stream to a process stream.  
e.g. `fs2.Pipe[F, Event, Nothing]`  
event handlers can send commands, that makes them process managers.

### Process manager
An event handler that may or may not maintain a state, and can send messages.  
Process managers are building blocks of communication, choreography and also orchestration.  

### Saga
A process manager that involves in a long running business process, that potentially models a transactional behavior which involves compensating actions.


## Tactical design

```scala mdoc:plantuml:mindmap
* Aggregate root
** id
** state
*** (value object +)
*** (entity +)
**** id
**** value object
```

### Value object
An immutable value that its value is its purpose. modeled as simply types, e.g. enums, opaque types, primitive types.  

### Entity
A value that has identity, and can be changed without losing its identity. modeled as case classes that have identity. entity field are mostly value objects.

### Aggregate
A collection of entities, that together form a boundary, that is the unit of transactions. they are portions of state of the state machine (automaton).

### Aggregate root
the root entity in an aggregate, that can be referenced from outside. this is the state machine entire state, which has a unique id that identifies its event stream too.
