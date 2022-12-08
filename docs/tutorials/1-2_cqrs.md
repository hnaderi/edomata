# CQRS style

## Imports

```scala mdoc
import edomata.core.*
import edomata.syntax.all.* // for convenient extension methods
import cats.implicits.* // to make life easier
```

## Domain layer
### EitherNec

If you are not familiar with `EitherNec` from cats, it is basically a type alias like the following
```scala
type EitherNec[L, R] = Either[NonEmptyChain[L], R]
```

which obviously shows that it behaves exactly like `Either`; 

Examples:  

```scala mdoc:to-string
val e1 = Right(1)
val e2 = "Missile Launched!".asRight
val e3 = "No remained missiles to launch!".leftNec
```

either is composable  
```scala mdoc:to-string
val e4 = e1.map(_ * 2)
val e5 = e2 >> e1
```  

You can also use for-comprehension:  

```scala mdoc:to-string
val e6 = for {
  a <- e4
  b <- e5
} yield a + b

```

for more [examples see here](https://typelevel.org/cats/datatypes/either.html) and also [here](https://typelevel.org/cats/datatypes/validated.html)

### Modeling

Let's use what we've learned so far to create an overly simplified model of a food delivery system.  
Assume we have the following business requirements:

- User must be able to place order  
- Line cook gets notified of new orders  
- Line cook allocates food to a cook
- User gets notified of order status  
- Kitchen can report that food is ready
- Delivery gets notified of ready foods
- Delivery allocates food to a delivery unit
- Delivery can mark an order as delivered
- User can submit a rating of her experience

> I'll use scala 3 enums for modeling ADTs, as they are neat and closer to what modeling is all about; but you can use `sealed trait`s and normal `case class`es too

Let's start with aggregate root which is the order here:

```scala
enum Order {
  case Empty
  case Placed(food: String, address: String, status: OrderStatus = OrderStatus.New)
  case Delivered(rating: Int)
}
```

and order status

```scala mdoc
enum OrderStatus {
  case New
  case Cooking(cook: String)
  case WaitingToPickUp
  case Delivering(unit: String)
}
```
we'll continue with modeling rejection scenarios that also came from event storming:  

```scala mdoc
enum Rejection {
  case ExistingOrder
  case NoSuchOrder
  case InvalidRequest // this should be more fine grained in real world applications
}
```


No we can use `EitherNec` to write our domain logic:  

```scala mdoc
import edomata.core.*
import cats.implicits.*
import cats.data.ValidatedNec

enum Order {
  case Empty
  case Placed(food: String, address: String, status: OrderStatus = OrderStatus.New)
  case Delivered(rating: Int)
  
  def place(food: String, address: String) = this match {
    case Empty => Placed(food, address).asRight
    case _ => Rejection.ExistingOrder.leftNec
  }
  
  def markAsCooking(cook: String) = this match {
    case st@Placed(_, _, OrderStatus.New) => st.copy(status = OrderStatus.Cooking(cook)).asRight
    case _ => Rejection.InvalidRequest.leftNec
  }
  
  // other logics from business
}
```

### DomainModel

In order to complete modeling we must also define transitions (the famous event sourcing fold!) and our starting point, for doing so we use `DomainModel`, which is a helper class that creates the required stuff for the next steps:

```scala mdoc
object Order extends CQRSModel[Order, Rejection] {
  def initial = Empty
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
Order.Empty.place("kebab", "home")
Order.Placed("pizza", "office").markAsCooking("chef")
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

### Stomaton
A `Stomaton` is an state-driven automata that can do the following:  

- read or modify the current state
- ask what is requested (e.g. command message)
- perform side effects
- decide (using EitherNec, return a value or reject with one or more rejections)
- notify (emit notifications, integration events, ...)
- output a value

`Stomaton`s are composable, so you can assemble them together, change them or treat them like normal data structures. 

for creating our first `Stomaton`, we need to model 2 more ADTs; commands, and notifications.

```scala mdoc
enum Command {
  case Place(food: String, address: String)
  case MarkAsCooking(cook: String)
  case MarkAsCooked
  case MarkAsDelivering(unit: String)
  case MarkAsDelivered
  case Rate(score: Int)
}

enum Notification {
  case Received(food: String)
  case Cooking
  case Cooked
  case Delivering
  case Delivered
}
```

and we can create our first service:

```scala mdoc
object OrderService extends Order.Service[Command, Notification] {
  import cats.Monad

  def apply[F[_] : Monad]: App[F, Unit] = App.router{
    case Command.Place(food, address) => for {
      ns <- App.modifyS(_.place(food, address))
      _ <- App.publish(Notification.Received(food))
    } yield ()
    case Command.MarkAsCooking(cook: String) => for {
      ns <- App.modifyS(_.markAsCooking(cook))
      _ <- App.publish(Notification.Cooking)
    } yield ()
    case _ => ??? // other command handling logic
  }
}

```

That's it! we've just written our first `Stomaton`.

### Testing an stomaton
As said earlier, everything in Edomata is just a normal value, and you can treat them like normal data.  

```scala mdoc:silent
import java.time.Instant

// as we've written our service definition in a tagless style, 
// we are free to provide any type param that satisfies required type-classes
val srv = OrderService[cats.Id] // or any other effect type

val scenario1 = srv.run(
  CommandMessage(
    id = "cmd id",
    time = Instant.MIN,
    address = "aggregate id",
    payload = Command.Place("taco", "home")
  ),
  Order.Empty    // state to run command on
)
```

and you can assert on response easily

```scala mdoc:to-string
scenario1.result
scenario1.notifications
```


## What's next?
So far we've created our program definitions, in order to run them as a real application in production, we need to compile them using a backend; which I'll discuss in the [next chapter](2_backends.md)

