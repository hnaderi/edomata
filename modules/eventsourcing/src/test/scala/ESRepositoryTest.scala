package edomata.eventsourcing

import cats.data.NonEmptyChain
import cats.data.Validated
import cats.effect.IO
import cats.effect.Resource
import munit.CatsEffectSuite
import cats.implicits.*

import java.time.OffsetDateTime

class ESRepositoryTest extends CatsEffectSuite {

  private val fold: Fold[Long, Long, String] = e =>
    s =>
      Validated.condNec(e > s, e + s, s"Must be bigger than current value $s")

  private val repo = for {
    sn <- InMemSnapshotStore[IO, String, AggregateState[Long]]
    jr <- InMemJournal[IO, String, Long]
  } yield MyESRepository[IO, String, Long, Long, String](
    sn,
    jr,
    io.odin.Logger.noop,
    0,
    fold
  )

  private val time = OffsetDateTime.MIN

  test("Initial") {
    for {
      r <- repo
      s <- r.get("a")
    } yield assertEquals(s, AggregateState(0, 0L))
  }

  test("First") {
    for {
      r <- repo
      _ <- r.append("a", time, 0, NonEmptyChain(1, 2, 4))
      s <- r.get("a")
      l <- r.history("a").compile.toList
    } yield {
      assertEquals(s, AggregateState(3, 7L))
      assertEquals(l.size, 3)
      assertEquals(l.last.payload, 7L)
      assertEquals(l.last.metadata.version, 3L)
    }
  }

  test("Second") {
    for {
      r <- repo
      _ <- r.append("a", time, 0, NonEmptyChain(1, 2, 4))
      s1 <- r.get("a")
      _ <- r.append("a", time, s1.version, NonEmptyChain(8))
      s2 <- r.get("a")
      l <- r.history("a").compile.toList
    } yield {
      assertEquals(s1, AggregateState(3, 7L))
      assertEquals(l.size, 4)
      assertEquals(l.last.payload, 15L)
      assertEquals(l.last.metadata.version, 4L)
    }
  }

  test("get must be last state in history") {
    for {
      r <- repo
      _ <- r.append("a", time, 0, NonEmptyChain(1, 2, 4))
      s <- r.get("a")
      l <- r.history("a").compile.toList
    } yield assertEquals(l.last.payload, s.state)
  }

  test("Must not allow events that cannot be folded") {
    for {
      r <- repo
      att <- r.append("a", time, 0, NonEmptyChain(1, 2, 3)).attempt
      s <- r.get("a")
    } yield {
      assertEquals(s, AggregateState(0, 0L))
      att match {
        case Left(MyESRepository.Error.LogicError(l)) =>
          assertEquals(l.length, 1L)
        case _ => fail("Did not reject invalid event!")
      }
    }
  }
}
