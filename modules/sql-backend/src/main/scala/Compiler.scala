package edomata.backend

import cats.data.NonEmptyChain
import edomata.core.*

trait Compiler[F[_], E, N] {
  def append(
      ctx: RequestContext[?, ?],
      events: NonEmptyChain[E],
      notifications: Seq[N]
  ): F[Unit]

  def notify(
      ctx: RequestContext[?, ?],
      notifications: NonEmptyChain[N]
  ): F[Unit]
}
