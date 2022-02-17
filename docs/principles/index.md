# Principles

## Event driven system
Events are first class citizens of the system, and in a pure eda system, events are reasons; meaning that nothing happens unless there is exactly one event for it. events are not meant to be source of data transfer as they are very lean and most of the you need more than one event to reach a conclusion, but sometimes are used that way for pragmatic reasons.  

## Event sourcing
It is a persistence strategy that takes EDA to its extreme, where any change to any data is due to events.
it's litmus test would be something like this:  
```scala mdoc:plantuml
start

:system changes its state;
if (why?) then (I have its reason as an event)
 #palegreen:verified as
 event sourced;
else (It depends)
 #pink:not event sourced;
```  

Note that this is not an architecture as opposed to EDA which is, ES is considered a local persistence strategy, and it must be completely transparent and isolated from external systems.

## CQRS
it is the idea of applying CQS principle to whole sub-systems, meaning that responsibility for handling commands are completely separated from handling queries. it is an extremely useful pattern for following scenarios:
- when there is a need to have more than one representation of data, and different databases for each of them
- when business logic is totally irrelevant to views that users need or business logic is to complex to be mixed with other irrelevant responsibilities.

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
using events to communicate; this by far most spread misconception! that is message passing, not event sourcing.

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
