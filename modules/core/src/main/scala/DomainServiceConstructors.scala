package edomata.core

import cats.Applicative
import cats.Monad
import cats.data.NonEmptyChain
import cats.data.ValidatedNec
import cats.implicits.*

import java.time.Instant

import Domain.*
type CTX[D] = RequestContext2[CommandFor[D], StateFor[D], MetadataFor[D]]
type SM[F[_], D, T] =
  ServiceMonad[F, CTX[D], RejectionFor[D], EventFor[D], NotificationFor[D], T]

private[core] trait DomainServiceConstructors {
  def command[F[_]: Monad, Domain]: SM[F, Domain, CommandFor[Domain]] =
    ServiceMonad.map(_.command)
  def metadata[F[_]: Monad, Domain]: SM[F, Domain, MetadataFor[Domain]] =
    ServiceMonad.map(_.metadata)
  def aggregateId[F[_]: Monad, Domain]: SM[F, Domain, String] =
    ServiceMonad.map(_.aggregateId)
  def messageId[F[_]: Monad, Domain]: SM[F, Domain, String] =
    ServiceMonad.map(_.id)
  def state[F[_]: Monad, Domain]: SM[F, Domain, StateFor[Domain]] =
    ServiceMonad.map(_.state)
}
