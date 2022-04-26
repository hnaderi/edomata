# Features

## Flexible data models

there are several data models in `core` module, which are flexible enough to use for other scenarios.  
the list consists of:  

- @:api(edomata.core.Decision) a state machine which represent decisions and has `MonadError` and is also traversable.
- @:api(edomata.core.DecisionT) a transformer that runs an effect that yields a `Decision`
- @:api(edomata.core.Response) a state machine which consists of a `Decision` and has publishing capability like a writer monad.
- @:api(edomata.core.Action) a transformer which runs an effect that yields a `Response`
- @:api(edomata.core.Edomaton) a program that reads a value, runs an effect and yields a `Response`

## Convenient syntax and friendly type inference

Working with a tri-functor might become tedious if you want to fill every type parameter, 
doing so for a functor with 5 or 6 type parameters would be enormously tedious and boring as hell;
edomata provide convenient syntax which helps exactly with that!  
For creating large typed functors like `Edomaton`, you can easily use provided dsls, or extension methods which due to dependent typing, solves the problem of passing too many types. as a rule of thumb you will pass each of your domain types once.

## Starter kits

Some opinionated types and models are provided to help you while learning or testing. as mentioned in [rationale](../introduction.md#rationale), this library is not meant to replace anything or provide a framework; its most important goal is to provide a working, production ready example of what an event sourced functional system would look like, to help you design and learn. in doing so types we have types like:

- @:api(edomata.core.CommandMessage) a standard way of modeling command messages.
- @:api(edomata.core.MessageMetadata) a standard way of modeling metadata in a message passing system.
- @:api(edomata.core.RequestContext) a standard way of representing a request in a functional event sourced system.

you can create your own models and use them with or without other data models provided in edomata.
