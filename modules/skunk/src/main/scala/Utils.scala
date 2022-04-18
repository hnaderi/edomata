package edomata.backend

import cats.Functor
import cats.effect.kernel.Clock
import cats.implicits.*

import java.time.OffsetDateTime
import java.time.ZoneOffset

private def currentTime[F[_]: Functor](using
    clock: Clock[F]
): F[OffsetDateTime] =
  clock.realTimeInstant.map(_.atOffset(ZoneOffset.UTC))
