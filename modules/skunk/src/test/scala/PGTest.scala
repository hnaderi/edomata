package edfsm.eventsourcing

import cats.effect.IO
import munit.CatsEffectSuite
import edfsm.common.pgPersistence.PGFixture

abstract class PGTest extends CatsEffectSuite {
  protected val SUT = ResourceSuiteLocalFixture(
    "Journal",
    PGFixture.pool
  )

  override def munitFixtures = List(SUT)
}
