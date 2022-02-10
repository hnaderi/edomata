package edfsm.eventsourcing

import cats.effect.IO
import cats.effect.Resource
import munit.CatsEffectSuite
import edfsm.common.pgPersistence.PGFixture
import edfsm.common.pgPersistence.Database
import java.time.ZoneOffset
import cats.data.NonEmptyChain
import java.util.UUID

class PGJournalTest extends CatsEffectSuite {

  private val schemaName = "test"
  private val journal: Resource[IO, Journal[IO, String, Int]] = for {
    session <- PGFixture.single
    _ <- Resource.eval(session.execute(Database.Schema.create(schemaName)))
    _ <- Resource.eval(PGJournal.setup(schemaName, session))
  } yield PGJournal[IO, Int](schemaName, session)

  private val SUT = ResourceSuiteLocalFixture(
    "Journal",
    journal
  )

  override def munitFixtures = List(SUT)

  private val newId = IO(UUID.randomUUID.toString)
  private val currentTime = IO.realTimeInstant.map(_.atOffset(ZoneOffset.UTC))

  test("Must be reversible") {
    for {
      id <- newId
      l1 <- SUT().readStream(id).compile.toList
      now <- currentTime
      _ <- SUT().append(id, now, 0L, NonEmptyChain(1, 2, 3))
      l2 <- SUT().readStream(id).compile.toList
    } yield {
      assertEquals(l1, Nil)
      assertEquals(l2.map(_.payload), List(1, 2, 3))
      assert(
        l2.map(_.metadata.time).forall(_ == now),
        "Event times must be from append"
      )
      assert(
        l2.map(_.metadata.stream).forall(_ == id),
        "Received events from other streams"
      )
      assertEquals(l2.map(_.metadata.version), List(1L, 2L, 3L))
    }
  }

  test("Partial streams must be after specified version") {
    val events = NonEmptyChain(1, (2 to 10).toSeq: _*)
    for {
      id <- newId
      l1 <- SUT().readStream(id).compile.toList
      now <- currentTime
      _ <- SUT().append(id, now, 0L, events)
      l2 <- SUT().readStreamAfter(id, 7L).compile.toList
    } yield {
      assertEquals(l1, Nil)
      assertEquals(l2.map(_.payload), List(8, 9, 10))
      assert(
        l2.map(_.metadata.time).forall(_ == now),
        "Event times must be from append"
      )
      assert(
        l2.map(_.metadata.stream).forall(_ == id),
        "Received events from other streams"
      )
      assertEquals(l2.map(_.metadata.version), List(8L, 9L, 10L))
    }
  }

}
