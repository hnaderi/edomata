package edfsm.backend

import cats.effect.IO
import cats.effect.Resource
import cats.implicits.*
import edfsm.eventsourcing.AggregateState
import io.circe.Codec
import io.circe.Decoder
import io.circe.Encoder
import munit.CatsEffectSuite
import natchez.Trace.Implicits.noop
import edfsm.common.pgPersistence.PGFixture
import edfsm.protocols.command.CommandMessage
import skunk.net.protocol.Describe

import java.util.UUID

import CommandHandlerTest.*

class CommandHandlerTest extends CatsEffectSuite {

  val myFixture = ResourceSuiteLocalFixture(
    "Service",
    handler
  )
  val entityId = ResourceSuiteLocalFixture(
    "entity id",
    Resource.eval(newId)
  )

  override def munitFixtures = List(myFixture, entityId)

  test("Concurrent commands on a single entity") {
    val n = 10
    for {
      s1 <- myFixture().dal.use(_.readFromJournal(entityId()))
      msgs <- (1 to n).toList.traverse(_ => newCommandFor(entityId()))
      res <- IO.parTraverseN(10)(msgs)(myFixture().handler)
      s2 <- myFixture().dal.use(_.readFromJournal(entityId()))
    } yield {
      assertEquals(res, List.fill(n)(Right(())))
      assertEquals(s1, AggregateState(0, 0L))
      assertEquals(s2, AggregateState(n, n.toLong))
    }
  }

  test("Sequential commands on a single entity") {
    val n = 10
    for {
      entId <- newId
      s1 <- myFixture().dal.use(_.readFromJournal(entId))
      msgs <- (1 to n).toList.traverse(_ => newCommandFor(entId))
      res <- msgs.traverse(myFixture().handler)
      s2 <- myFixture().dal.use(_.readFromJournal(entId))
    } yield {
      assertEquals(res, List.fill(n)(Right(())))
      assertEquals(s1, AggregateState(0, 0L))
      assertEquals(s2, AggregateState(n, n.toLong))
    }
  }

  test("Concurrent commands to different entities") {
    for {
      msgs <- (1 to 10).toList.traverse(i =>
        newCommandFor(('a' + i).toChar.toString)
      )
      res <- IO.parTraverseN(10)(msgs)(myFixture().handler)
    } yield assertEquals(res, List.fill(10)(Right(())))
  }

  test("Get") {
    myFixture().dal
      .use(_.readFromJournal(entityId()))
      .flatTap(IO.println)
      .map(assertEquals(_, AggregateState(10, 10L)))
  }

  test("Single command") {
    for {
      id <- newId
      msg <- newCommandFor(id)
      res <- myFixture().handler(msg)
      s <- myFixture().dal.use(_.readFromJournal(id))
    } yield assertEquals(s, AggregateState(1, 1L))
  }
}

object CommandHandlerTest {
  private val logger = io.odin.consoleLogger[IO]()

  private val newId = IO(UUID.randomUUID.toString)

  final case class SUT(
      handler: CommandHandler[IO, DummyDomain],
      dal: Resource[IO, ServiceDAL[IO, DummyDomain]]
  )

  val handler: Resource[IO, SUT] = for {
    pool <- PGFixture.pool
    builder <- Resource.pure(ServiceBuilder(pool, logger))
    handler <- Resource.eval(
      builder.build[DummyDomain](
        "dummy",
        0L,
        DummyDomain.transition,
        DummyDomain[IO]
      )
    )
    dal = PGServiceDAL.from[IO, DummyDomain](
      "dummy",
      0L,
      DummyDomain.transition,
      logger
    )(pool)
  } yield SUT(handler, dal)

  private def newCommandFor(address: String) = for {
    now <- IO.realTimeInstant
    id <- newId
  } yield CommandMessage(id = id, now, address = address, 1)
}
