# Principles

## Message passing system

Systems (usually based on the [actor model](#actor-model)) that communicate through message passing: interactions are done by sending encapsulated data packets, which may have any arbitrary intents and semantics. This decouples execution stages both in time and space, so execution can continue on a different machine, or at a different time on the same or another machine.

## Event-driven system

A specific form of message passing that always uses events as messages. Events are first-class citizens of the system, and in a pure EDA system events are reasons and facts: nothing happens unless there is exactly one event for it.

## Event sourcing

A persistence strategy that takes EDA to its extreme, where any change to any data is due to events. A component where all changes to its data satisfy the following criterion is event sourced:

```text
the system changes its state
  └── why?
       ├── "I have its reason as an event"  → verified as event sourced
       └── "it depends"                     → not event sourced
```

Note that this is not an architecture, as opposed to EDA which is. ES is a local persistence strategy, and it must be totally transparent to, and isolated from, external systems.

## CQRS

The idea of applying the CQS principle to whole sub-systems: the responsibility for handling commands is totally separated from handling queries. It is a useful pattern when:

- there is a need for more than one representation of data, with different databases for each;
- business logic is irrelevant to the views users need, or is too complex to be mixed with other responsibilities.

This is also a local pattern, not an architecture, and almost never a replacement for CRUD systems. CQRS does not have to mean event sourcing, commands, events, read sides, sagas or asynchronous processing.

## Actor model

A mathematical model of concurrent computation that treats the actor as the universal primitive. In response to a message it receives, an actor can:

* make local decisions;
* create more actors;
* send more messages;
* determine how to respond to the next message.

Actors may modify their own private state, but can only affect each other indirectly through messaging.

# Misconceptions and anti-patterns

## Communicating using the journal

Using events to communicate. This is by far the most widespread misconception: that is message passing, not event sourcing.

## "CQRS architecture"

CQRS is a local application pattern. A system "with CQRS architecture" conveys no meaning and shows a form of cargo-cult design.

## "Event-sourced architecture"

Same as above. In properly designed systems (with DDD principles at least) domains are separated by their ubiquitous language, which is the boundary of meaning where data becomes information. That data is private; not isolating it causes many other misconceptions in DDD / ES design.

## Redundant events

Events carry facts; redundant events signal wrong usage or wrong event structure.

## Dependent events

Events are facts. If they depend on other events to be true or not, they are not events.

## Passive-aggressive events

`IHaveDoneMyWorkSoYouShouldDoThisJobEventHappened`, `DoThisJob`: a sign of not understanding EDA, and of fitting an event-driven system to an imperative, micromanaging mindset.
