package edomata.examples

import cats.effect.IO
import cats.implicits.*
import edomata.backend.Backend
import edomata.core.*

import Domain.*

object Example1 {
  enum Event {
    case Opened
    case Received(i: Int)
    case Closed
  }
  enum Rejection {
    case Unknown
  }

  sealed trait Counter extends Model[Counter, Event, Rejection] { self =>
    def transition = {
      case Event.Opened      => self.valid
      case Event.Received(i) => self.valid
      case Event.Closed      => self.valid
    }
  }
  case object Empty extends Counter
  final case class Open(i: Int) extends Counter
  case object Closed extends Counter

  enum Updates {
    case Updated()
    case Closed()
  }

  type CounterDomain =
    HasModel[Counter] And
      HasCommand[String] And
      HasNotification[Updates]

  val dsl = Edomaton.of[CounterDomain]

  def app: EdomatonOf[IO, CounterDomain, Unit] = dsl.router {
    case "" => dsl.read[IO].map(_.command).map(_.deriveMeta).void
    case _  => dsl.reject(Rejection.Unknown)
  }

  def backend: Backend.Of[IO, CounterDomain] = ???

  val service = app.compile(backend.compiler)

  val publisher = backend.outbox.read.evalMap(i =>
    IO.println(i) >>
      backend.outbox.markAsSent(i)
  )

  val resp = service(
    CommandMessage("abc", ???, "a", "hello", MessageMetadata("user"))
  )

  val h = backend.repository.history("a").printlns

}
