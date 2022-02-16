package edomata.eventsourcing

import cats.data.NonEmptyChain
import cats.effect.IO
import cats.effect.Resource
import munit.CatsEffectSuite
import munit.ScalaCheckSuite

import java.time.OffsetDateTime

class InMemJournalTest extends CatsEffectSuite {

  private val journal = InMemJournal[IO, String, Long]

  private val sut = ResourceSuiteLocalFixture(
    "Journal",
    Resource.eval(journal)
  )

  override def munitFixtures = List(sut)

  test("Sanity") {
    sut().readAll.compile.toList.map(l => assertEquals(l, Nil))
  }

  test("Must reject append with old version") {
    for {
      sut <- journal
      _ <- sut.append("a", OffsetDateTime.MIN, 0, NonEmptyChain(1, 2, 3))
      _ <- sut.append("a", OffsetDateTime.MIN, 2, NonEmptyChain(1)).attempt
      _ <- sut.append("a", OffsetDateTime.MIN, 4, NonEmptyChain(1)).attempt
      evs <- sut.readStream("a").evalMap(IO.println).compile.toList
    } yield assertEquals(evs.size, 3)
  }

  test("Must be sorted") {
    for {
      sut <- journal
      _ <- sut.append("a", OffsetDateTime.MIN, 0, NonEmptyChain(1, 2, 3))
      evs <- sut.readStream("a").compile.toList
    } yield assertEquals(evs.map(_.metadata.seqNr), List(1L, 2L, 3L))
  }

}
