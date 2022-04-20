package edomata.backend

import cats.Functor
import cats.MonadError
import cats.effect.kernel.Clock
import cats.implicits.*
import skunk.data.Completion

import java.time.OffsetDateTime
import java.time.ZoneOffset

private def currentTime[F[_]: Functor](using
    clock: Clock[F]
): F[OffsetDateTime] =
  clock.realTimeInstant.map(_.atOffset(ZoneOffset.UTC))

extension [F[_]](self: F[Completion])(using F: MonadError[F, Throwable]) {
  private[backend] def assertInserted(size: Int): F[Unit] = self.flatMap {
    case Completion.Insert(i) =>
      if i == size then F.unit
      else
        F.raiseError(
          BackendError.PersistenceError(
            s"expected to insert exactly $size, but inserted $i"
          )
        )
    case other =>
      F.raiseError(
        BackendError.PersistenceError(
          s"expected to receive insert response, but received: $other"
        )
      )
  }
  private[backend] def assertInserted: F[Unit] = assertInserted(1)
}
