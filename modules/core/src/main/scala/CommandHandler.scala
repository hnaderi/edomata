package edomata.core

import cats.Monad
import cats.data.EitherNec
import cats.data.NonEmptyChain
import cats.implicits.*

import java.time.OffsetDateTime
import java.time.ZoneOffset

trait CommandHandler[F[_], C, S, E, R, N, M] {
  def onRequest[T](
      cmd: CommandMessage[C, M]
  )(f: RequestContext2[C, Model.Of[S, E, R], M, R] => F[T]): F[T]

  def onAccept(
      ctx: RequestContext2.Valid[C, Model.Of[S, E, R], M, R],
      events: NonEmptyChain[E],
      notifications: Seq[N]
  ): F[Unit]

  def onIndecisive(
      ctx: RequestContext2.Valid[C, Model.Of[S, E, R], M, R],
      notifications: Seq[N]
  ): F[Unit]

  def onReject(
      ctx: RequestContext2.Valid[C, Model.Of[S, E, R], M, R],
      notifications: Seq[N],
      reasons: NonEmptyChain[R]
  ): F[Unit]

  def onConflict(
      ctx: RequestContext2[C, Model.Of[S, E, R], M, R],
      reasons: NonEmptyChain[R]
  ): F[Unit]
}
