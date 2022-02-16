package edomata.backend.skunk

import cats.effect.IO
import edomata.backend.skunk.PGFixture
import munit.CatsEffectSuite

abstract class PGTest extends CatsEffectSuite {
  protected val SUT = ResourceSuiteLocalFixture(
    "Journal",
    PGFixture.pool
  )

  override def munitFixtures = List(SUT)
}
