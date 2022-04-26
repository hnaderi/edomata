# Design goals

## ideas

- functional programming is cool!
- message passing is key to reliable/resilient systems.
- event driven design is necessary for natural domain modeling.
- DDD is a great design guideline.
- state machines are building blocks of distributed systems.

## to achieve

- Expressive domain modeling
- Test driven development and general testing must be as easy as validating against literal numbers
- serve as a CQRS chassis
- we should be able to run a specific program with different interpreters with different characteristics.

## non goals

- general tools for all the scenarios  
this library is designed specifically for event-driven/event-sourced applications with a DDD mindset in mind, it would be suitable for those specific problems, and probably a bad fit for others.  
- performance  
while it is considered as much as possible, to eliminate unnecessary works and do works as efficient as possible; performance has lower priority than being expressive. if you know how improve specific performance problems I would be very happy to discuss the matter, or more happy if you do a PR!
