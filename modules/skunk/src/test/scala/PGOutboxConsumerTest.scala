package edfsm.common.notification

import cats.effect.IO
import cats.effect.Resource
import cats.effect.std.CountDownLatch
import cats.implicits.*
import edfsm.eventsourcing.PGTest
import fs2.Stream
import munit.CatsEffectSuite
import edfsm.common.pgPersistence.Database
import edfsm.common.pgPersistence.PGFixture

import java.time.OffsetDateTime
import java.time.ZoneOffset
import java.util.UUID

import PGOutboxConsumerTest.*

class PGOutboxConsumerTest extends PGTest {
  private def consumer = PGOutboxConsumer(SUT(), logger)
  private def setup(ns: String) =
    SUT().use(s =>
      s.execute(Database.Schema.create(ns)) >> PGOutboxService
        .setup[IO, String](s, ns)
    )

  private def consume(ns: String) = for {
    ref <- IO.ref(List.empty[OutboxItem[String]])
    con = Stream(())
      .through(
        consumer.pipe[String](ns, it => ref.update(_ :+ it))
      )
    _ <- con.compile.drain
    out <- ref.get
  } yield out

  private def write[T](ns: String)(f: OutboxService[IO, String] => IO[T]) =
    val wr = SUT()
      .flatTap(_.transaction)
      .evalMap(s => PGOutboxService[IO, String](s, ns, logger))
    wr.use(f)

  private def currentTime =
    IO.realTimeInstant
      .map(_.atOffset(ZoneOffset.UTC))

  private def write1(ns: String, data: String) = write(ns) { o =>
    for {
      time <- currentTime
      _ <- o.outbox(data, time)
    } yield OutboxedItem(time, data)
  }

  private def writeRandom(ns: String) = newId.flatMap(write1(ns, _))

  private def writeRandomAndWait(ns: String, latch: CountDownLatch[IO]) = for {
    data <- Resource.eval(newId)
    now <- Resource.eval(currentTime)
    l <- Resource.eval(CountDownLatch[IO](1))
    _ <- write(ns)(_.outbox(data, now) >> l.release >> latch.await).background
    _ <- Resource.eval(l.await)
  } yield OutboxedItem(now, data)

  private def assertConsume(ns: String, data: OutboxedItem*)(using
      munit.Location
  ) =
    consume(ns)
      .map(_.map(it => OutboxedItem(it.time, it.data)))
      .assertEquals(
        data.toList,
        "Must consume outboxed messages"
      )

  private def assertConsumeX(ns: String, item: OutboxedItem, seqnr: Long)(using
      munit.Location
  ) =
    consume(ns)
      .assertEquals(
        List(OutboxItem(seqnr, item.time, item.data)),
        "Must consume outboxed messages"
      )

  final case class OutboxedItem(time: OffsetDateTime, data: String)

  private def assertEmptyConsume(ns: String) =
    consume(ns).assertEquals(Nil, "Must not read again after publishing")

  test("Consuming outboxed messages") {
    for {
      ns <- newNS
      _ <- setup(ns)
      d1 <- writeRandom(ns)
      d2 <- writeRandom(ns)
      _ <- assertConsume(ns, d1, d2)
      _ <- assertEmptyConsume(ns)
      d3 <- writeRandom(ns)
      _ <- assertConsume(ns, d3)
    } yield ()
  }

  test("Consuming out of order outboxed messages") {
    for {
      ns <- newNS
      _ <- setup(ns)
      latch <- CountDownLatch[IO](1)
      _ <- writeRandomAndWait(ns, latch).use(d1 =>
        for {
          d2 <- writeRandom(ns)
          _ <- assertConsumeX(ns, d2, 2)
          _ <- assertEmptyConsume(ns)
          _ <- latch.release
          _ <- assertConsumeX(ns, d1, 1)
        } yield ()
      )
    } yield ()
  }

}

object PGOutboxConsumerTest {
  private val logger = io.odin.consoleLogger[IO]()
  private val newId = IO(UUID.randomUUID.toString)
  private val newNS = newId.map(_.replaceAll("-", "_")).map(ns => s"test_$ns")

}
