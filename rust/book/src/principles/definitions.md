# Definitions

Useful definitions in the context of DDD / CQRS / ES, kept as generic as possible; some are also mapped to this library to help understanding.

## Message

A packet of data that can carry some information.

```text
message
├── event
│   ├── domain event
│   ├── integration event
│   └── ...
├── command
└── document
```

### Command message

A message whose intent is to request a change of system behaviour. Commands are named with imperative verbs (`ReceivePayment`, `ConfirmOrder`, `MarkAsDelivered`) and may be rejected by the system according to its logic, policies and state. In Edomata: `CommandMessage<C>`.

### Event message

A message whose intent is to capture a fact. Events are named with past participles (`OrderReceived`, `AssetSecured`, `PaymentReceived`) and cannot be rejected: they are facts about what happened. In Edomata: the `E` of a `Decision`, journaled as `EventMessage<E>`.

### Document message

A message whose purpose is to transfer information without any other intent; consumers decide how to act. Documents are named with nouns (`HealthReport`, `InvestorWeeklyReport`, `ShippingStatistics`). They often represent aggregated data and may be large (do not send documents of that size to message brokers).

## Components

### Projection

An aggregated data structure per event stream that maintains a specific viewpoint from historical events.

### Query

A request to ask the system for data. Queries are idempotent by nature, cannot change system behaviour, and can usually be cached.

### Event stream

The history of events from the beginning, in chronological order.

### Command handler

A component that receives a command message, decides what to do based on domain logic, and acts upon it. In Edomata it is a function `CommandMessage<C> → Future<Result<Result<(), NonEmpty<R>>, BackendError>>` (`DomainService<C, R>`), obtained by compiling an `Edomaton` or `Stomaton` with a backend.

### Event handler

A component that receives a subset of events from one or more streams and runs side effects; modelled as a transformation of an event stream into a process stream (`futures::Stream`). Event handlers can send commands, which makes them process managers. They should be as deterministic as possible: if an event handler sends messages, the message ids must be reproducible (not random UUIDs), which helps with workflows and delivery guarantees. `edomata-broker` derives its message ids from the source and the sequence number for this reason.

### Process manager

An event handler that may maintain a state and can send messages. Process managers are the building blocks of communication, choreography and orchestration.

### Saga

A process manager involved in a long-running business process, potentially modelling a transactional behaviour with compensating actions.

## Tactical design

```text
aggregate root
├── id
└── state
    ├── value object (+)
    └── entity (+)
        ├── id
        └── value object
```

### Value object

An immutable value whose value is its purpose; modelled as simple types: enums, newtypes, primitive types.

### Entity

A value that has an identity and can change without losing it; modelled as structs with an id whose fields are mostly value objects.

### Aggregate

A collection of entities that together form a boundary, the unit of transactions; portions of the state of the state machine (automaton).

### Aggregate root

The root entity of an aggregate, the one referenced from outside: the entire state of the state machine, with a unique id that also identifies its event stream.
