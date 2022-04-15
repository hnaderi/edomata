package edomata.examples

import cats.effect.IO
import cats.effect.kernel.Resource
import cats.implicits.*
import edomata.backend.*
import edomata.core.*
import skunk.Session

object Example1 {
  enum Event {
    case Opened
    case Received(i: Int)
    case Closed
  }
  enum Rejection {
    case Unknown
  }

  enum Counter {
    case Empty
    case Open(i: Int)
    case Closed
  }
  object Counter extends DomainModel[Counter, Event, Rejection] {
    def initial = Empty
    def transition = {
      case Event.Opened      => _.valid
      case Event.Received(i) => _.valid
      case Event.Closed      => _.valid
    }
    extension (self: Counter) {
      def receive(i: Int): Decision[Rejection, Event, Counter] = self.perform(
        self match {
          case Empty   => Decision.accept(Event.Opened, Event.Received(i))
          case Open(_) => Decision.accept(Event.Received(i))
          case Closed  => Decision.reject(Rejection.Unknown)
        }
      )
    }
  }

  enum Updates {
    case Updated()
    case Closed()
  }

  val ns = Counter.Empty.perform(Decision.accept(Event.Opened))

  val CounterDomain = Counter.domain[String, Updates]

  private val dsl = CounterDomain.dsl

  def app: dsl.App[IO, Unit] = dsl.router {
    case "" => dsl.read[IO].map(_.command).map(_.deriveMeta).void
    case "receive" =>
      for {
        s <- dsl.state[IO]
        ns <- dsl.perform(s.receive(2))
      } yield ()
    case _ => dsl.reject(Rejection.Unknown)
  }

  def backend = CounterDomain.skunkBackend[IO](???)

  val doobieBLD = DoobieBackend[IO]()
  val backend2 = doobieBLD.buildNoSetup(CounterDomain, "counter")

  val service = app.compile(backend.compiler)

  val publisher = backend.outbox.read.evalMap(i =>
    IO.println(i) >>
      backend.outbox.markAsSent(i)
  )

  val resp = service(
    CommandMessage("abc", ???, "a", "hello")
  )

  val h = backend.repository.history("a").printlns

}
