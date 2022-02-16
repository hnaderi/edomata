package edomata.backend

import cats.Monad
import cats.data.Kleisli
import cats.data.ValidatedNec
import edomata.core.Action
import edomata.core.Decision

import FSMDefinition.*

type DomainLogic2[F[_], Domain, T] =
  Kleisli[[tt] =>> DomainAR[F, Domain, tt], RequestContext[Domain], T]

type DomainService2[F[_], Domain] = DomainLogic2[F, Domain, Unit]
object DomainService2 {
  def apply[F[_]: Monad, Domain](
      f: CommandFor[Domain] => DomainLogic2[F, Domain, Unit]
  ): DomainService2[F, Domain] = DomainLogic2.Builder[F, Domain]().router(f)
}

object DomainLogic2 {

  /** Builds a command router logic */
  def apply[F[_]: Monad, Domain, T](
      f: CommandFor[Domain] => DomainLogic2[F, Domain, T]
  ): DomainLogic2[F, Domain, T] = Builder[F, Domain]().router(f)

  final case class Builder[F[_]: Monad, Domain]() {
    type App[T] = DomainLogic2[F, Domain, T]
    def request: App[RequestContext[Domain]] =
      Kleisli(r => Action.pure(r))
    def eval[T](f: F[T]): App[T] =
      Kleisli.liftF(Action.liftF(f))
    def lift[T](
        action: DomainAR[F, Domain, T]
    ): App[T] =
      Kleisli.liftF(action)
    def message: App[DomainCommand[Domain]] =
      Kleisli(r => Action.pure(r.command))
    def command: App[CommandFor[Domain]] =
      message.map(_.payload)
    def state: App[StateFor[Domain]] = request.map(_.state)
    def aggregateId: App[String] = request.map(_.aggregateId)

    def pure[T](t: T): App[T] = lift(Action.pure(t))
    def void: App[Unit] = lift(Action.void)

    def accept(
        e: InternalEventFor[Domain],
        es: InternalEventFor[Domain]*
    ): App[Unit] =
      lift(Action.accept(e, es: _*))

    def reject[T](
        reason: RejectionFor[Domain],
        otherReasons: RejectionFor[Domain]*
    ): App[T] = lift(Action.reject(reason, otherReasons: _*))

    def publish(
        notifications: ExternalEventFor[Domain]*
    ): App[Unit] = lift(Action.publish(notifications: _*))

    def validate[T](
        validation: ValidatedNec[RejectionFor[Domain], T]
    ): App[T] = lift(Action.validate(validation))

    def router[T](f: CommandFor[Domain] => App[T]): App[T] = command.flatMap(f)
  }

}
