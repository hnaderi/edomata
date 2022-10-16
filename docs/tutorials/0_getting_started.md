# Getting started

## Add to your build

```scala
libraryDependencies += "dev.hnaderi" %% "edomata-core" % "@VERSION@"
```

or for scala.js
```scala
libraryDependencies += "dev.hnaderi" %%% "edomata-core" % "@VERSION@"
```

## Imports

```scala mdoc
import edomata.core.*
import edomata.syntax.all.* // for convenient extension methods
```

## Layers of abstraction

```scala mdoc:plantuml:wbs
* layers
** domain

*** is pure
*** ensures aggregate invariants
-** does not change frequently\n in well established businesses
-** models an aggregate root in DDD

** service

*** may involve side effects
-** is very minimal and mostly glueing
*** must be idempotent
-** models application services\nand command handlers in DDD
```

## Domain layer
### Decision
It all starts with decisions!  
Decision models programs that decide in an event-driven context, these programs are pure and can:

- accept events  
- reject with errors  
- stay indecisive!  

```scala mdoc:plantuml
State InDecisive : output
State Accepted : events, output
State Rejected : errors

InDecisive -up-> Accepted : event
InDecisive -> InDecisive : bind
InDecisive --> Rejected : error(s)
Accepted -> Accepted : accumulates
Accepted --> Rejected : error(s)
```

Examples:  

```scala mdoc:to-string
val d1 = Decision(1)
val d2 = Decision.accept("Missile Launched!")
val d3 = Decision.reject("No remained missiles to launch!")
```

You can also use the following syntax if you import syntax modules  

```scala mdoc:to-string
import edomata.syntax.all.*

1.asDecision
"Missile Launched!".accept
"No remained missiles to launch!".reject
```

decisions are composable  
```scala mdoc:to-string
val d4 = d1.map(_ * 2)
val d5 = d2 >> d1
val d6 = d5 >> d3
```

You can also use for-comprehension:  

```scala mdoc:to-string
val d7 = for {
  i <- Decision.pure(1)
  _ <- Decision.accept("A") // accepting one event
  _ <- Decision.accept("B", "C") // accepting several events
  j <- Decision.acceptReturn(i * 2)("D", "E") // accepting several events and returning a value
} yield i + j

```

or in presence of cats syntax:  
```scala mdoc:to-string
import cats.implicits.* 

val d8 = (d1, d7).mapN(_ + _)
val d9 = List.range(1, 5).traverse(Decision.accept(_))
val d10 = Decision(List.range(1, 5)).sequence[List, Int]
```

### Modeling

Let's use what we've learned so far to create an overly simplified model of a bank account.  
Assume we have the following business requirements:

- We must be able to open an account, if we did not have used that account name before  
- We can deposit/withdraw some amount of assets to an open account  
- We must be able to close an account, if it is settled (meaning its balance is zero)  

We'll start by modeling domain events first, that we have from event storming or other kinds of design practices:  

> I'll use scala 3 enums for modeling ADTs, as they are neat and closer to what modeling is all about; but you can use `sealed trait`s and normal `case class`es too

```scala mdoc:reset
enum Event {
  case Opened
  case Deposited(amount: BigDecimal)
  case Withdrawn(amount: BigDecimal)
  case Closed
}
```  

we'll continue with modeling rejection scenarios that also came from event storming:  

```scala mdoc
enum Rejection {
  case ExistingAccount
  case NoSuchAccount
  case InsufficientBalance
  case NotSettled
  case AlreadyClosed
  case BadRequest
}
```

no we must model an aggregate root:

```scala
enum Account {
  case New
  case Open(balance: BigDecimal)
  case Close
}
```

No we can use `Decision` to write our domain logic:  

```scala mdoc
import edomata.core.*
import edomata.syntax.all.*
import cats.implicits.*
import cats.data.ValidatedNec

enum Account {
  case New
  case Open(balance: BigDecimal)
  case Close
  
  def open : Decision[Rejection, Event, Open] = this.decide { // 1
    case New => Decision.accept(Event.Opened)
    case _ => Decision.reject(Rejection.ExistingAccount)
  }.validate(_.mustBeOpen) // 2
  
  def close : Decision[Rejection, Event, Account] = this.perform(mustBeOpen.toDecision.flatMap { account => // 3, 4
    if account.balance == 0 then Event.Closed.accept
    else Decision.reject(Rejection.NotSettled)
  })
  
  def withdraw(amount: BigDecimal): Decision[Rejection, Event, Open] = this.perform(mustBeOpen.toDecision.flatMap { account => 
    if account.balance >= amount && amount > 0 
    then Decision.accept(Event.Withdrawn(amount))
    else Decision.reject(Rejection.InsufficientBalance) 
    // We can model rejections to have values, which helps a lot for showing error messages, but it's out of scope for this document
  }).validate(_.mustBeOpen)

  def deposit(amount: BigDecimal): Decision[Rejection, Event, Open] = this.perform(mustBeOpen.toDecision.flatMap { account =>
    if amount > 0 then Decision.accept(Event.Deposited(amount))
    else Decision.reject(Rejection.BadRequest)
  }).validate(_.mustBeOpen)
  
  private def mustBeOpen : ValidatedNec[Rejection, Open] = this match { // 5
    case o@Open(_) => o.validNec
    case New => Rejection.NoSuchAccount.invalidNec
    case Close => Rejection.AlreadyClosed.invalidNec
  }
}
```

1. `.decide` is an extension method from Edomata syntax that helps with deciding and transitioning to a new state which I'll describe below
2. `.validate` ensures that after applying decision events, we will end up in an `Open` state, and will return `Open` instead of simply `Account`
3. `.perform` like `.decide` takes a decision as an argument, and runs the decision on the current state to return new state, it is basically a helper for folding to let you reuse `transition` and not repeating yourself
4. `Decision` can be converted `.toValidated`, `.toOption` or `.toEither`.
5. as functional data structures are composable, you can extract common use cases like validations.

but you might say, domain logic is not just deciding, you must perform the actions you decided; and you are right, let's go to that part:

### DomainModel

In order to complete modeling we must also define transitions (the famous event sourcing fold!) and our starting point, for doing so we use `DomainModel`, which is a helper class that creates the required stuff for the next steps:

```scala mdoc
object Account extends DomainModel[Account, Event, Rejection] {
  def initial = New  // 1
  def transition = { // 2
    case Event.Opened => _ => Open(0).validNec
    case Event.Withdrawn(b) => _.mustBeOpen.map(s => s.copy(balance = s.balance - b)) // 3
    case Event.Deposited(b) => _.mustBeOpen.map(s => s.copy(balance = s.balance + b)) 
    case Event.Closed => _=> Close.validNec
  }

}
```

1. `initial` is the initial state of your domain model which is the first start in timeline changes, you can assume that it's like `None` in `Option[T]`. beside it being required for our tutorial purposes, defining an initial state has the following advantages:
- Model consistency; you are always working with your domain model, not with `Option[YourModel]`
- It enables to add new default values in the future, where your model evolves, and reduces the number of times when an upcasting or migration is required.
2. `transition` is the fold function that given an event, tells how to go to the next state. it's a function in `E => S => ValidatedNec[R, S]`. if an event is not possible to apply, we call it a conflict; and it might be due to programming errors, storage manipulation, or changing your transition to a conflicting logic with a history of already created timelines.

#### Tip
> Writing transition without using a library for optics and lenses would be cumbersome and not expressive enough for domain modeling; I suggest you to use the great [monocle](https://www.optics.dev/Monocle/) library which provides neat macros for lenses, and you don't even need to think about it twice after using it for the first time.

#### Thinking further
> Writing event-driven (and specifically event-sourced) domain models deal with explicitly modeling a timeline of facts, and one thing that you will face as soon you start modeling your first domain in such a context, is that not all timelines are valid as you are not always in the control of what you'll read from a journal; for example, in the example above, we might receive a Withdrawn event on `New` or `Closed` state (of course that does not happen unless there is a programming error or manipulated data, but we are speaking about possibility here), and you can't even find a balance to decrement! in non-functional settings where states are not modeled as ADTs, this problem is somewhat implicit, as it is hidden from eyes, but in modeling with ADTs, compiler slaps you in the face! just kidding :D, explicit modeling allows you to be more expressive and find logical problems very easily.


#### Info
> If you have a programming error in your folding which can cause conflicts, Edomata has your back! it won't let your program to reach a result where it can be persisted and corrupting data; we'll see this in the next chapters.


As simple as that!

### Testing domain model

As everything is pure and all the logic is implemented as programs that are values, you can easily test everything:  
```scala mdoc:to-string
Account.New.open
Account.Open(10).deposit(2)
Account.Open(5).close
Account.New.open.flatMap(_.close)
```

## Service layer
Domain models are pure state machines, in isolation, in order to create a complete service you need a way to:  

- store and load models
- interact with them
- possibly perform some side effects

you can do all that without Edomata, as everything in Edomata is just pure data and you can use it however you like; but here we'll focus on what Edomata has to offer.  
Edomata is designed around the idea of event-driven state machines,
and it's not surprising that the tools that it provides for building services are also event-driven state machines!
these state machines are like [actors](../principles/index.md#actor-model) that respond to incoming messages which are domain commands,
may possibly change state through emitting some events as we've seen in domain modeling above,
and possibly emit some other type of events for communication and integration with other services;
while doing so, they can also perform any side effects that are idempotent,
as these machines may be run several times in case of failure. that takes us to the next building block:

### Edomaton
An `Edomaton` is an event-driven automata (pl. Edomata) that can do the following:  

- read the current state
- ask what is requested (command message)
- perform side effects
- decide (as described in the decision above)
- notify (emit notifications, integration events, ...)
- output a value

`Edomaton`s are composable, so you can assemble them together, change them or treat them like normal data structures. 

#### Info
> an `Edomaton` is like an employee, while a `Decision` is like a business rule

for creating our first `Edomaton`, we need to model 2 more ADTs; commands, and notifications.

```scala mdoc
enum Command {
  case Open
  case Deposit(amount: BigDecimal)
  case Withdraw(amount: BigDecimal)
  case Close
}

enum Notification {
  case AccountOpened(accountId: String)
  case BalanceUpdated(accountId: String, balance: BigDecimal)
  case AccountClosed(accountId: String)
}
```

and we can create our first service:

```scala mdoc
object AccountService extends Account.Service[Command, Notification] {
  import cats.Monad

  def apply[F[_] : Monad] : App[F, Unit] = App.router {

    case Command.Open => for {
      ns <- App.state.decide(_.open)
      acc <- App.aggregateId
      _ <- App.publish(Notification.AccountOpened(acc))
    } yield ()

    case Command.Deposit(amount) => for {
      deposited <- App.state.decide(_.deposit(amount))
      accId <- App.aggregateId
      _ <- App.publish(Notification.BalanceUpdated(accId, deposited.balance))
    } yield ()

    case Command.Withdraw(amount) => for {
      withdrawn <- App.state.decide(_.withdraw(amount))
      accId <- App.aggregateId
      _ <- App.publish(Notification.BalanceUpdated(accId, withdrawn.balance))
    } yield ()

    case Command.Close => 
      App.state.decide(_.close).void
  }

}

```

That's it! we've just written our first `Edomaton`.

### Testing an edomaton
As said earlier, everything in Edomata is just a normal value, and you can treat them like normal data.  
`Edomaton`s take an environment, and work in that context to produce a result; 
using default DSL like what we've done in this tutorial creates `Edomaton`s that require a data type called `RequestContext` which is pretty standard modeling of a request context in an event-driven setup.

```scala mdoc
import java.time.Instant

val scenario1 = RequestContext(
  command = CommandMessage(
    id = "some random id for request",
    time = Instant.MIN,
    address = "our account id",
    payload = Command.Open
  ),
  state = Account.New
)
```

There are 2 ways for running an `Edomaton` to get its result:  

- Recommended way is to use `.execute` which takes input and returns processed results, which is used in real backends too.

```scala mdoc:to-string
// as we've written our service definition in a tagless style, 
// we are free to provide any type param that satisfies required type-classes

import cats.Id
val obtained = AccountService[Id].execute(scenario1)

// or even use a real IO monad if needed
import cats.effect.IO

AccountService[IO].execute(scenario1)
```
- or you can use `.run`, which takes input and returns raw `Response` model, which you can interpret however you like.

```scala mdoc
AccountService[Id].run(scenario1)
```

now we can easily assert our expectations using our favorite test framework

```scala 
assertEquals(
  obtained,
  EdomatonResult.Accepted(
    newState = Account.Open(0),
    events = ...
    notifications = ...
  )
)
```

## What's next?
So far we've created our program definitions, in order to run them as a real application in production, we need to compile them using a backend; which I'll discuss in the [next chapter](1_backends.md)
