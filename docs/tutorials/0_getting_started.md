# Getting started

## Install 

```scala
libraryDependencies += "io.github.hnaderi" %% "edomata-core" % "@VERSION@"
```

or for scala.js
```scala
libraryDependencies += "io.github.hnaderi" %%% "edomata-core" % "@VERSION@"
```

## Imports

```scala mdoc
import edomata.core.*
import edomata.syntax.all.* // for convenient extension methods
```

## Decision
It all start with decisions!  
Decision models programs that decide in a event driven context, these programs are pure and can:

- accept events  
- reject with errors  
- stay indecisive!  

```scala mdoc:to-string
val d1 = Decision(1)
val d2 = Decision.accept("Missile Launched!")
val d3 = Decision.reject("No remained missiles to launch!")
```

decisions are composable  
```scala mdoc:to-string
val d4 = d1.map(_ * 2)
val d5 = d2 >> d1
val d6 = d5 >> d3
```

you can also use for comprehensions:  
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

## Modeling

Let's use what we've learned so far to create an overly simplified model of a bank account.  
Assume we have the following business requirements:

- We must be able to open an account, if we did not have used that account name before  
- We can deposit/withdraw some amount of assets to an open account  
- We must be able to close an account, if it is settled (meaning its balance is zero)  

We'll start by modeling domain events first, that we have from event storming or other kind of design practices:  

> I'll use scala 3 enums for modeling ADTs, as they are neat and more close to what modeling is all about; but you can use `sealed trait`s and normal `case class`es too

```scala mdoc:reset-object
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
  case NoSettled
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
  
  def open : Decision[Rejection, Event, Account] = this.decide { // 1
    case New => Decision.accept(Event.Opened)
    case _ => Decision.reject(Rejection.ExistingAccount)
  }
  
  def close : Decision[Rejection, Event, Account] = this.perform(mustBeOpen.toDecision.flatMap { account => // 2, 3
    if account.balance == 0 then Decision.accept(Event.Closed)
    else Decision.reject(Rejection.NoSettled)
  })
  
  def withdraw(amount: BigDecimal): Decision[Rejection, Event, Account] = this.perform(mustBeOpen.toDecision.flatMap { account =>
    if account.balance >= amount && amount > 0 then Decision.accept(Event.Withdrawn(amount))
    else Decision.reject(Rejection.InsufficientBalance) 
    // We can model rejections to have values, which helps a lot for showing error messages, but it's out of scope for this document
  })

  def deposit(amount: BigDecimal): Decision[Rejection, Event, Account] = this.perform(mustBeOpen.toDecision.flatMap { account =>
    if amount > 0 then Decision.accept(Event.Deposited(amount))
    else Decision.reject(Rejection.BadRequest)
  })
  
  private def mustBeOpen : ValidatedNec[Rejection, Open] = this match { // 4
    case o@Open(_) => o.validNec
    case New => Rejection.NoSuchAccount.invalidNec
    case Close => Rejection.AlreadyClosed.invalidNec
  }
}
```

1. `.decide` is an extension method from edomata syntax that helps with deciding and transitioning to new state which I'll describe below
2. `.perform` like `.decide` takes a decision as argument, an runs the decision on current state to return new state, it is basically a helper for folding to let you reuse `transition` and not repeating yourself
3. `Decision` can be converted `.toValidated`, `.toOption` or `.toEither`.
4. as functional data structures are composable, you can extract common use cases like validations.

but you might say, domain logic is not just deciding, you must perform the actions you decided; and you are right, let's go to that part:

## DomainModel

In order to complete modeling we must also define transitions (the famous event sourcing fold!) and our starting point, for doing so we use `DomainModel`, which is a helper class that creates required stuff for next steps:

```scala mdoc
object Account extends DomainModel[Account, Event, Rejection]{
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
- It enables to add new default values in future, where your model evolves and reduces number of times where an upcasting or migration is required.
2. `transition` is the fold function that given an event, tells how to go to next state. it's a function in `E => S => ValidatedNec[R, S]`. if an event is not possible to apply, we call it a conflict; and it might be due to programming errors, storage manipulation, changing your transition to a conflicting logic with history of already created timelines.

#### Tip
> Writing transition without using a library for optics and lenses would be cumbersome and not expressive enough for domain modeling; I suggest you to use the great [monocle](https://www.optics.dev/Monocle/) library which provides neat macros for lenses, and you don't even need to think about it twice after using it for the first time.

#### Thinking further
> Writing event driven (and specifically event sourced) domain models deals with explicitly modeling a timeline of facts, and one thing that you will face as soon you start modeling your first domain in such a context, is that not all timelines are valid as you are not always in the control of what you'll read from a journal; for example, in the example above, we might receive a Withdrawn event on `New` or `Closed` state (of course that does not happen unless there is a programming error or manipulated data, but we are speaking about possibility here), and you can't even find a balance to decrement! in non functional settings where states are not modeled as ADTs, this problem is somewhat implicit, as it is hidden from eyes, but in modeling with ADTs, compiler slaps you in the face! just kidding :D, explicit modeling allows you to be more expressive and find logical problems very easily.


#### Info
> If you have a programming error in your folding which can cause conflicts, edomata has your back! it won't let your program to reach a result where it can be persisted and corrupting data; we'll see this in next chapters.


As simple as that!

## Testing

As everything is pure and all the logic is implemented as programs that are values, you can easily test everything:  
```scala mdoc:to-string
Account.New.open
Account.Open(10).deposit(2)
Account.Open(5).close
Account.New.open.flatMap(_.close)
```

## What's next?

So far you've seen what edomata has to offer for domain modeling, but what about application layer, persistence and other stuff? we'll continue with [Edomaton](1_hello_world.md), which is the building block for creating applications and services (and of course why this library is called Edomata)
