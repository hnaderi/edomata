/*
 * Copyright 2021 Hossein Naderi
 *
 * Licensed under the Apache License, Version 2.0 (the "License");
 * you may not use this file except in compliance with the License.
 * You may obtain a copy of the License at
 *
 *     http://www.apache.org/licenses/LICENSE-2.0
 *
 * Unless required by applicable law or agreed to in writing, software
 * distributed under the License is distributed on an "AS IS" BASIS,
 * WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
 * See the License for the specific language governing permissions and
 * limitations under the License.
 */

package edomata.examples.nr1

import cats.effect.IO
import cats.effect.IOApp
import cats.effect.kernel.Resource
import cats.implicits.*
import edomata.backend.*
import edomata.core.*
import edomata.syntax.all.*
import natchez.Trace.Implicits.noop
import skunk.Session
import upickle.default.*

import java.time.Instant
import scala.concurrent.duration.*

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

  def receive(i: Int): Decision[Rejection, Event, Counter] = this.perform(
    this match {
      case Empty   => Decision.accept(Event.Opened, Event.Received(i))
      case Open(_) => Decision.accept(Event.Received(i))
      case Closed  => Decision.reject(Rejection.Unknown)
    }
  )
}
object Counter extends DomainModel[Counter, Event, Rejection] {
  def initial = Empty
  def transition = {
    case Event.Opened      => _.valid
    case Event.Received(i) => _.valid
    case Event.Closed      => _.valid
  }
}

enum Updates {
  case Updated()
  case Closed()
}

object Application extends IOApp.Simple {

  val ns = Counter.Empty.perform(Decision.accept(Event.Opened))

  val CounterDomain = Counter.domain[String, Updates]

  private val dsl = CounterDomain.dsl

  def app: dsl.App[IO, Unit] = dsl.router {
    case "" => dsl.read[IO].map(_.command).map(_.deriveMeta).void
    case "receive" =>
      for {
        s <- dsl.state
        ns <- dsl.decide(s.receive(2))
        _ <- dsl.eval(IO.println(ns))
        _ <- dsl.publish(Updates.Updated())
      } yield ()
    case _ => dsl.reject(Rejection.Unknown)
  }

  // given BackendCodec[Event] = CirceCodec.jsonb(using ???, ???)
  given ReadWriter[Event.Opened.type] = macroRW
  given ReadWriter[Event.Received] = macroRW
  given ReadWriter[Event.Closed.type] = macroRW
  given ReadWriter[Event] = macroRW
  given ReadWriter[Updates.Updated] = macroRW
  given ReadWriter[Updates.Closed] = macroRW
  given ReadWriter[Updates] = macroRW

  given BackendCodec[Event] = UpickleCodec.jsonb
  given BackendCodec[Updates] = UpickleCodec.msgpack

  def backendRes(pool: Resource[IO, Session[IO]]) = SkunkBackend(pool)
    .builder(CounterDomain, "counter")
    // .persistedSnapshot(???, maxInMem = 200)
    .inMemSnapshot(200)
    .withRetryConfig(retryInitialDelay = 2.seconds)
    .build

  val database = Session
    .pooled[IO]("localhost", 5432, "postgres", "postgres", Some("postgres"), 10)

  val application = database.flatMap(backendRes).use { backend =>
    val service = backend.compile(app)

    service(
      CommandMessage("abc", Instant.now, "a", "receive")
    ).flatMap(IO.println)
  }

  def run: IO[Unit] = application
}
