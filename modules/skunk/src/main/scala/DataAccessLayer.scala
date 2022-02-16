package edomata.backend.skunk

import cats.MonadError
import cats.effect.Sync
import cats.implicits.*
import fs2.Stream
import skunk.Session
import skunk.*
import skunk.data.Completion
import skunk.exception.SkunkException
import skunk.implicits.*

import java.time.OffsetDateTime
import scala.io.Codec

object DataAccessLayer {
  def loadSQLFrom[F[_]](name: String)(implicit F: Sync[F]): F[String] =
    F.delay(
      scala.io.Source
        .fromResource(s"sql/$name.sql")(Codec.UTF8)
        .getLines()
        .mkString("\n")
    )

  def setupQueryFor[F[_]: Sync](name: String, session: Session[F]): F[Unit] =
    for {
      init <- loadSQLFrom(name)
      q = sql"#$init".command
      _ <- session.execute(q)
    } yield ()

}

import Completion.*
import DAOError.*

extension [F[_]](f: F[Completion])(using F: MonadError[F, Throwable]) {
  def assertUpdated: F[Unit] = assertUpdatedSize(1)
  def assertUpdatedSize(size: Int): F[Unit] = f.flatMap {
    case Update(rows) if rows == size => F.unit
    case Update(rows)                 => F.raiseError(PartialUpdate(size, rows))
    case other                        => F.raiseError(NotAnUpdate(other))
  }
  def assertInserted: F[Unit] = assertInsertedSize(1)
  def assertInsertedSize(size: Int): F[Unit] =
    f.flatMap {
      case Insert(rows) if rows == size => F.unit
      case Insert(rows) => F.raiseError(PartialInsertion(size, rows))
      case other        => F.raiseError(NotAnInsert(other))
    }.adaptErr { case SqlState.UniqueViolation(ex) => ExistingId(ex) }
}
