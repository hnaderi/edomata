# Principles

## Message passing system
Systems (usually based on [Actor model]) that communicate through message passing, meaning that interactions are done by sending encapsulated data packets,
which may have any arbitrary intents and semantics.
this may be used to decouple execution stages both in time and space, so execution can continue on a different machine, or a different time on the same or another machine.

## Event driven system
A specific form of message passing that always uses events as messages.
Events are first class citizens of the system, and in a pure eda system, events are reasons and facts; meaning that nothing happens unless there is exactly one event for it.

## Event sourcing
It is a persistence strategy that takes EDA to its extreme, where any change to any data is due to events.
For any component, where all changes to its data satisfy the following criteria, we call it event sourced.  
```scala mdoc:plantuml
start

:system changes its state;
if (why?) then (I have its reason as an event)
 #palegreen:verified as
 event sourced;
else (It depends)
 #pink:not event sourced;
```  

Note that this is not an architecture as opposed to EDA which is, ES is considered a local persistence strategy, and it must be totally transparent and isolated from external systems.

## CQRS
it is the idea of applying CQS principle to whole sub-systems, meaning that responsibility for handling commands are totally separated from handling queries. it is a useful pattern for following scenarios:
- when there is a need to have more than one representation of data, and different databases for each of them
- when business logic is totally irrelevant to views that users need or business logic is too complex to be mixed with other irrelevant responsibilities.

Note that this is also a local pattern, not an architecture, and almost always is not a replacement for crud systems.  
Also CQRS doesn't have to mean doing event sourcing, introducing commands, event, read sides, sagas, async processing and so forth.

## Actor model
The actor model in computer science is a mathematical model of concurrent computation that treats actor as the universal primitive of concurrent computation. In response to a message it receives, an actor can: 

* make local decisions
* create more actors
* send more messages
* determine how to respond to the next message received.

Actors may modify their own private state, but can only affect each other indirectly through messaging.

# Misconceptions and anti-patterns

## Communicating using journal
using events to communicate; this is by far most spread misconception! that is message passing, not event sourcing.

## CQRS architecture
as described earlier, CQRS is not a system architecture, it is a local application architecture or pattern; having a system with CQRS architecture not only does not convey any meaning, it also shows a form a cargo cult programming and design.

## Event sourced architecture
as described in CQRS architecture! in a properly designed systems (using DDD principles at least) domains are separated by domain language (ubiquitous language) which is the boundary of meanings,
this is where data in presence of a domain context becomes meaningful (a.k.a. information) and its semantics are defined, 
this is a totally private data and not isolating it is the cause of a lot of other misconceptions in DDD/ES designing.

## Redundant events
Events are meant to carry facts, having redundant events can signal wrong usage of events or wrong event structure

## Dependent events
Events are facts, if they are dependent on other events to be true or not, they are not events  

## Passive aggressive events
`IHaveDoneMyWorkSoShouldDoThisJobEventHappened`, `DoThisJob`. this is a sign of not understanding EDA, and trying to fit an event driven system to an imperative and/or micromanaging mindset.
