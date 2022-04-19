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

  test("Must add to cache and set") {
    for {
      c <- Cache.lru[IO, String, Unit](1)
      s <- IO.ref(HashSet.empty[String])
      ics = CommandStore.InMemoryCommandStore(c, s)
      _ <- ics.append(cmd("a"))
      _ <- c.get("a").assertEquals(Some(()))
      _ <- s.get.assertEquals(Set("a"))
      _ <- ics.contains("a").assertEquals(true)
    } yield ()
  }

  test("Must remove from set when cache size is reached") {
    for {
      c <- Cache.lru[IO, String, Unit](1)
      s <- IO.ref(HashSet.empty[String])
      ics = CommandStore.InMemoryCommandStore(c, s)
      _ <- ics.append(cmd("a"))
      _ <- ics.append(cmd("b"))
      _ <- c.get("a").assertEquals(None)
      _ <- c.get("b").assertEquals(Some(()))
      _ <- s.get.assertEquals(Set("b"))
      _ <- ics.contains("a").assertEquals(false)
      _ <- ics.contains("b").assertEquals(true)
    } yield ()
  }
}
object InMemoryCommandStoreSuite {
  def cmd(i: String) = CommandMessage(i, Instant.MAX, "", ())
}
