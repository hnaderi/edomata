package edomata.backend

import cats.data.Chain
import cats.data.NonEmptyChain
import cats.effect.IO
import cats.implicits.*
import edomata.core.*
import munit.CatsEffectSuite

import java.time.Instant
import scala.collection.immutable.HashSet

import InMemoryCommandStoreSuite.*

class InMemoryCommandStoreSuite extends CatsEffectSuite {

  test("Must add") {
    for {
      ics <- CommandStore.inMem[IO](1)
      _ <- ics.append(cmd("a"))
      _ <- ics.contains("a").assertEquals(true)
    } yield ()
  }

  test("Must remove when cache size is reached") {
    for {
      ics <- CommandStore.inMem[IO](1)
      _ <- ics.append(cmd("a"))
      _ <- ics.append(cmd("b"))
      _ <- ics.contains("a").assertEquals(false)
      _ <- ics.contains("b").assertEquals(true)
    } yield ()
  }
}
object InMemoryCommandStoreSuite {
  def cmd(i: String) = CommandMessage(i, Instant.MAX, "", ())
}
