package edfsm.common.notification

import cats.effect.IO
import cats.effect.Resource
import cats.implicits.*
import edfsm.eventsourcing.PGTest
import fs2.Stream
import munit.CatsEffectSuite
import edfsm.common.notification.PGOutboxNotification

import scala.concurrent.duration.*

import PGNotificationTest.*

class PGNotificationTest extends PGTest {

  private def notification(ns: String) =
    Stream.resource(PGOutboxNotification(SUT(), logger)).flatMap(_.stream(ns))

  test("Must notify listeners on different sessions") {
    notification("test").as(1).holdResource(0).use { ref =>
      SUT().use(s =>
        for {
          ch <- PGOutboxNotification.getChannel(s, "test")
          _ <- ch.notify("")
          _ <- ref.discrete
            .dropWhile(_ == 0)
            .head
            .timeout(5.seconds)
            .compile
            .lastOrError
            .assertEquals(1)
        } yield ()
      )
    }
  }

}

object PGNotificationTest {
  private val logger = io.odin.consoleLogger[IO]()
}
