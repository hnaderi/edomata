package edomata.core

import cats.Applicative
import cats.Functor
import cats.Monad
import cats.data.NonEmptyChain
import cats.data.ValidatedNec
import cats.implicits.*

import java.time.Instant

import Domain.*
type CTX[D] =
  RequestContext2.Valid[CommandFor[D], StateFor[D], MetadataFor[
    D
  ], RejectionFor[D]]
type SM[F[_], D, T] =
  ServiceMonad[F, CTX[D], RejectionFor[D], EventFor[D], NotificationFor[D], T]

private[core] final class DomainServiceConstructors[
    C,
    S,
    E,
    R,
    N,
    M
](
    private val dummy: Boolean = true
) extends AnyVal {
  def command[F[_]: Applicative]
      : ServiceMonad[F, RequestContext2.Valid[C, S, M, Nothing], R, E, N, C] =
    ServiceMonad.map(_.command.payload)
  def metadata[F[_]: Applicative]
      : ServiceMonad[F, RequestContext2.Valid[C, S, M, Nothing], R, E, N, M] =
    ServiceMonad.map(_.command.metadata)
  def aggregateId[F[_]: Applicative]: ServiceMonad[
    F,
    RequestContext2.Valid[C, S, M, Nothing],
    R,
    E,
    N,
    String
  ] =
    ServiceMonad.map(_.command.address)
  def messageId[F[_]: Applicative]: ServiceMonad[
    F,
    RequestContext2.Valid[C, S, M, Nothing],
    R,
    E,
    N,
    String
  ] =
    ServiceMonad.map(_.command.id)
  def state[F[_]: Applicative]
      : ServiceMonad[F, RequestContext2.Valid[C, S, M, Nothing], R, E, N, S] =
    ServiceMonad.map(_.state)
}

object ServiceMonadHelpers {
  def state[F[_]: Monad, C, M, S, R, E, N, T]
      : ServiceMonad[F, RequestContext2.Valid[C, S, M, R], R, E, N, S] =
    ServiceMonad.read.map(_.state)
  def aggregateId[F[_]: Monad, C, M, S, R, E, N, T]
      : ServiceMonad[F, RequestContext2.Valid[C, S, M, R], R, E, N, String] =
    ServiceMonad.read.map(_.command.address)
  def metadata[F[_]: Monad, C, M, S, R, E, N, T]
      : ServiceMonad[F, RequestContext2.Valid[C, S, M, R], R, E, N, M] =
    ServiceMonad.read.map(_.command.metadata)
  def messageId[F[_]: Monad, C, M, S, R, E, N, T]
      : ServiceMonad[F, RequestContext2.Valid[C, S, M, R], R, E, N, String] =
    ServiceMonad.read.map(_.command.id)
  def command[F[_]: Monad, C, M, S, R, E, N, T]
      : ServiceMonad[F, RequestContext2.Valid[C, S, M, R], R, E, N, C] =
    ServiceMonad.read.map(_.command.payload)
}
