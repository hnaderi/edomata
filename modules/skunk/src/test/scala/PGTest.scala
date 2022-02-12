package edfsm.backend.skunk

import cats.effect.IO
import edfsm.backend.skunk.PGFixture
import munit.CatsEffectSuite

abstract class PGTest extends CatsEffectSuite {
  protected val SUT = ResourceSuiteLocalFixture(
    "Journal",
    PGFixture.pool
  )

  override def munitFixtures = List(SUT)
}
