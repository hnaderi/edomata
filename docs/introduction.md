# Introduction

This is a light weight, polymorphic, purely functional library to implement event-driven automatons in scala.  
It is light weight in the terms of simplicity, so you are always dealing with a few primitive abstractions that are very intuitive.
It is purely functional and built for typelevel ecosystem in mind, however it does not enforce any structure or decision on you and you can use it however you like.  

## Goals
provide a solution to implement event driven systems, using domain driven design, by focusing on domain logic and building an automaton to represent the language.

## Alternatives
There are a lot of attempts with huge success in this area, but every one of them has some caveats and I wanted to attack the problem in a different angle.  

### Akka and Akka persistence
it is a very strong and well established ecosystem, but it has a few drawbacks:

* incompatible ecosystem with FP
* too low level
* easy to use, but not simple
* tries to solve a lot of problems at once, which can lead to mixing domain logic and implementation details if not used carefully
* in case of event sourcing, it does mislead somehow, as sourcing inputs does not make a system event sourced
* you can't compose or reuse actors easily
* it is somewhat complex and needs a lot of specific experience in Akka itself
* not friendly with purely functional programs as it requires shims and wrappers every where

however, it has a large ecosystem and community, and it should be your first consideration if none of the drawbacks above make any sense to you.

### [Aecor](https://github.com/notxcain/aecor)
this is a very nice purely functional wrapper (with brilliant design) around Akka and Akka persistence, solves the problem with composablity and purely functional programming, but leaves us with yet another level of indirection which makes the mix more complex.

### Axon
While this is not a direct alternative, it tries to do some of things this library is meant for.

### Comparison

|             | Edomata                                | Axon                | Akka and friends              | Aecor                                |
|-------------|----------------------------------------|---------------------|-------------------------------|--------------------------------------|
| Paradigm    | Purely functional                      | OOP                 | Imperative/message passing    | Purely functional                    |
| Style       | Haskell-ish scala                      | Java                | Erlang-ish java               | MTL                                  |
| Usage       | Library                                | Framework           | Framework/Platform            | Library + akka                       |
| Persistence | Custom interpreters*                   | Axon server         | Akka persistence backends     | like akka                            |
| Testing     | Trivial                                | Test library        | Test library                  | Trivial + like akka                  |
| Runtime     | JVM/JS/Native                          | JVM                 | JVM                           | JVM                                  |
| Focus       | Expressive event driven state machines | CQRS+Event-sourcing | Actor model/Erlang OTP on JVM | Purely functional behaviors for Akka |
| Ecosystem   | Typelevel                              | Java                | Lightbend                     | Typelevel over akka                  |
|             |                                        |                     |                               |                                      |

* Production ready Postgres based backends are provided in a doobie and skunk flavor.


### Your home grown toolbox
Event sourcing does not require any framework and almost always they mess up the work, as mentioned by Greg Young and other pioneers of ES/CQRS, and it does not need a long way to go in order to reach this conclusion, it is sufficient to use one of the available frameworks for a real project and you will reach the same conclusion (if you care for simplicity); so it is almost always better to develop your home grown toolbox and utilities, right?!  
IMO yes actually, and you are asking so what's the point of this library if you think so?!!!  
that seems like a paradox, but I'll explain it in [Rationale]

## Rationale
Designing DDD systems requires a lot of experience, which is really hard to convey in text books, as the problems that DDD solves are problems that we face when deepen in a specific domain, and try to discover it; which is a hard and time consuming process that can't be simulated in a comprehensive way in a book; and most development efforts rely on using opinionated frameworks which will lead to problems mentioned above, and worst of all, most of the literature for this are written with OOP in mind and does not transfer to FP easily and requires way more experience in both area. also there are a lot of spread misconception about what is event sourcing and how and when to use it all over the internet, which will lead to increasing this gap.  
This library is meant to help with this problem, so it can be both a way to show how those ideas can be mapped to FP, and also probably a handy toolbox acting like a seed to grow your own home grown toolbox.

## Next
- [Getting started](tutorials/0_getting_started.md)
- [Principles](principles/index.md)
- [FAQ](other/faq.md)
- [Design goals](about/design_goals.md)
- [List of modules](other/modules.md)
