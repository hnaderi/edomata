package edomata.core

import cats.Applicative
import cats.Monad
import cats.implicits.*

import Domain.*

private final class DomainDSL[D](
    private val dummy: Boolean = true
) extends AnyVal {
  inline def pure[F[_]: Monad, T](
      t: T
  ): EdomatonOf[F, D, T] =
    Edomaton.pure(t)

  inline def unit[F[_]: Monad]: EdomatonOf[F, D, Unit] =
    Edomaton.unit

  inline def liftF[F[_], T](
      f: F[Response[RejectionFor[D], EventFor[D], NotificationFor[D], T]]
  ): EdomatonOf[F, D, T] = Edomaton.liftF(f)

  inline def lift[F[_]: Applicative, T](
      f: Response[RejectionFor[D], EventFor[D], NotificationFor[D], T]
  ): EdomatonOf[F, D, T] = Edomaton.lift(f)

  inline def eval[F[_]: Applicative, T](
      f: F[T]
  ): EdomatonOf[F, D, T] = Edomaton.eval(f)

  inline def run[F[_]: Applicative, T](
      f: ContextOf[D] => F[T]
  ): EdomatonOf[F, D, T] = Edomaton.run(f)

  inline def map[F[_]: Applicative, T](
      f: ContextOf[D] => T
  ): EdomatonOf[F, D, T] =
    Edomaton.map(f)

  inline def read[F[_]: Applicative]: EdomatonOf[F, D, ContextOf[D]] =
    Edomaton.read

  inline def publish[F[_]: Applicative](
      ns: NotificationFor[D]*
  ): EdomatonOf[F, D, Unit] =
    Edomaton.publish(ns: _*)

  inline def reject[F[_]: Applicative, T](
      r: RejectionFor[D],
      rs: RejectionFor[D]*
  ): EdomatonOf[F, D, T] =
    Edomaton.reject(r, rs: _*)

  inline def perform[F[_]: Applicative, T](
      d: Decision[RejectionFor[D], EventFor[D], T]
  ): EdomatonOf[F, D, T] =
    Edomaton.perform(d)

  def state[F[_]: Monad]: EdomatonOf[F, D, StateFor[D]] =
    Edomaton.read.map(_.state)

  def aggregateId[F[_]: Monad]: EdomatonOf[F, D, String] =
    Edomaton.read.map(_.command.address)

  def metadata[F[_]: Monad]: EdomatonOf[F, D, MetadataFor[D]] =
    Edomaton.read.map(_.command.metadata)

  def messageId[F[_]: Monad]: EdomatonOf[F, D, String] =
    Edomaton.read.map(_.command.id)

  def command[F[_]: Monad]: EdomatonOf[F, D, CommandFor[D]] =
    Edomaton.read.map(_.command.payload)

  def router[F[_]: Monad, T](
      f: CommandFor[D] => EdomatonOf[F, D, T]
  ): EdomatonOf[F, D, T] =
    command.flatMap(f)

}

object DomainDSL {
  def apply[D]: DomainDSL[D] = new DomainDSL()
}

extension (unused: Edomaton.type) {
  def of[D]: DomainDSL[D] = new DomainDSL()
}
