package edfsm.common
package pgPersistence

import cats.effect.IO
import skunk.Session
import cats.effect.Resource
import natchez.Trace.Implicits.noop
import cats.implicits.*

object PGFixture {
  val pgHost = sys.env.getOrElse("POSTGRES_HOST", "localhost")
  val pgPort = sys.env.get("POSTGRES_PORT").map(_.toInt).getOrElse(5432)
  val pgUser = sys.env.getOrElse("POSTGRES_USERNAME", "postgres")
  val pgPass = sys.env.get("POSTGRES_PASSWORD").orElse(Some("postgres"))

  val pool: Resource[IO, Resource[IO, Session[IO]]] = pooled(10)

  def pooled(size: Int) = Session
    .pooled[IO](
      host = pgHost,
      port = pgPort,
      user = pgUser,
      database = "postgres",
      password = pgPass,
      max = size
    )

  val single = pooled(1).flatten
}
