package edomata.core

import cats.Applicative
import cats.Functor
import cats.Monad
import cats.data.NonEmptyChain
import cats.data.ValidatedNec
import cats.implicits.*

import java.time.Instant

import Domain.*
type CTX[D] = RequestContext2[CommandFor[D], StateFor[D], MetadataFor[D]]
type SM[F[_], D, T] =
  ServiceMonad[F, CTX[D], RejectionFor[D], EventFor[D], NotificationFor[D], T]

final class DomainServiceConstructors[C, S, E, R, N, M] private[core] (
    private val dummy: Boolean = true
) extends AnyVal {
  def command[F[_]: Applicative]
      : ServiceMonad[F, RequestContext2[C, S, M], R, E, N, C] =
    ServiceMonad.map(_.command)
  def metadata[F[_]: Applicative]
      : ServiceMonad[F, RequestContext2[C, S, M], R, E, N, M] =
    ServiceMonad.map(_.metadata)
  def aggregateId[F[_]: Applicative]
      : ServiceMonad[F, RequestContext2[C, S, M], R, E, N, String] =
    ServiceMonad.map(_.aggregateId)
  def messageId[F[_]: Applicative]
      : ServiceMonad[F, RequestContext2[C, S, M], R, E, N, String] =
    ServiceMonad.map(_.id)
  def state[F[_]: Applicative]
      : ServiceMonad[F, RequestContext2[C, S, M], R, E, N, S] =
    ServiceMonad.map(_.state)
}
