package edomata.backend.skunk

import cats.effect.IO
import cats.effect.Resource
import cats.implicits.*
import natchez.Trace.Implicits.noop
import skunk.Session

object PGFixture {
  val pgHost = sys.env.getOrElse("POSTGRES_HOST", "localhost")
  val pgPort =
    sys.env.get("POSTGRES_PORT").flatMap(_.toIntOption).getOrElse(5432)
  val pgUser = sys.env.getOrElse("POSTGRES_USER", "postgres")
  val pgPass = sys.env.get("POSTGRES_PASSWORD").orElse(Some("postgres"))
  val pgDb = sys.env.getOrElse("POSTGRES_DB", "postgres")

  val pool: Resource[IO, Resource[IO, Session[IO]]] = pooled(10)

  def pooled(size: Int) = Session
    .pooled[IO](
      host = pgHost,
      port = pgPort,
      user = pgUser,
      database = pgDb,
      password = pgPass,
      max = size
    )

  val single = pooled(1).flatten
}
