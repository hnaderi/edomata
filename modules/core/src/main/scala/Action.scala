package edomata.core

import cats.Applicative
import cats.Functor
import cats.Monad
import cats.data.ValidatedNec
import cats.implicits._
import edomata.core.Decision.Accepted
import edomata.core.Decision.InDecisive
import edomata.core.Decision.Rejected

final case class ActionResult[R, E, N, T](
    decision: Decision[R, E, T],
    notifications: Seq[N] = Nil
)

final case class Action[F[_], R, E, N, A](run: F[ActionResult[R, E, N, A]]) {
  def map[B](f: A => B)(using F: Functor[F]): Action[F, R, E, N, B] =
    Action(F.map(run)(d => d.copy(decision = d.decision.map(f))))

  def flatMap[R2 >: R, E2 >: E, N2 >: N, B](
      f: A => Action[F, R2, E2, N2, B]
  )(using M: Monad[F]): Action[F, R2, E2, N2, B] =
    Action {
      M.flatMap(run) { case ActionResult(decision, notifs) =>
        decision match {
          case e1 @ Accepted(events1, result) =>
            f(result).run.map { res =>
              res.decision match {
                case e2 @ Accepted(events2, _) =>
                  ActionResult(
                    e2.copy(events = events1 ++ events2),
                    notifs ++ res.notifications
                  )
                case InDecisive(result) =>
                  ActionResult(
                    e1.copy(result = result),
                    notifs ++ res.notifications
                  )
                case r: Rejected[R2, E2, B] => ActionResult(r.copy(), Nil)
              }
            }
          case InDecisive(result) =>
            f(result).run.map(res =>
              res.decision match {
                case r: Rejected[R2, E2, B] => res
                case r =>
                  res.copy(notifications = notifs ++ res.notifications)
              }
            )
          case r: Rejected[R, E, A] => M.pure(ActionResult(r.copy(), notifs))
        }
      }
    }

  def as[B](b: B)(using F: Functor[F]): Action[F, R, E, N, B] = map(_ => b)

  /** Clears all notifications so far */
  def reset(using F: Functor[F]): Action[F, R, E, N, A] =
    Action(run.map(_.copy(notifications = Nil)))

  /** Adds notification without considering decision state */
  def publish(ns: N*)(using F: Functor[F]): Action[F, R, E, N, A] =
    Action(
      run.map(res => res.copy(notifications = res.notifications ++ ns))
    )
}

object Action extends ActionConstructors with ActionCatsInstances

sealed transparent trait ActionConstructors {
  def lift[F[_], R, E, N, T](
      t: ActionResult[R, E, N, T]
  )(using F: Applicative[F]): Action[F, R, E, N, T] =
    Action(F.pure(t))

  def liftD[F[_], R, E, N, T](
      t: Decision[R, E, T]
  )(using F: Applicative[F]): Action[F, R, E, N, T] =
    lift(ActionResult(t))

  def pure[F[_], R, E, N, T](
      t: T
  )(using F: Applicative[F]): Action[F, R, E, N, T] =
    lift(ActionResult(Decision.pure(t)))

  def void[F[_]: Applicative, R, E, N]: Action[F, R, E, N, Unit] = pure(())

  def liftF[F[_], R, E, N, T](
      f: F[T]
  )(using F: Functor[F]): Action[F, R, E, N, T] =
    Action(F.map(f)(d => ActionResult(Decision.pure(d))))

  def validate[F[_]: Applicative, R, E, N, T](
      validation: ValidatedNec[R, T]
  ): Action[F, R, E, N, T] =
    liftD(Decision.validate(validation))

  def accept[F[_]: Applicative, R, E, N](
      ev: E,
      evs: E*
  ): Action[F, R, E, N, Unit] =
    liftD(Decision.accept(ev, evs: _*))

  def reject[F[_]: Applicative, R, E, N, T](
      reason: R,
      otherReasons: R*
  ): Action[F, R, E, N, T] =
    liftD(Decision.reject(reason, otherReasons: _*))

  def publish[F[_]: Applicative, R, E, N](
      ns: N*
  ): Action[F, R, E, N, Unit] =
    lift(ActionResult(Decision.unit, ns))
}

sealed transparent trait ActionCatsInstances {
  type DT[F[_], R, E, N] = [T] =>> Action[F, R, E, N, T]

  given [F[_], R, E, N](using F: Functor[F]): Functor[DT[F, R, E, N]] with
    def map[A, B](fa: Action[F, R, E, N, A])(
        f: A => B
    ): Action[F, R, E, N, B] = fa.map(f)

  given [F[_], R, E, N](using F: Monad[F]): Monad[DT[F, R, E, N]] with
    override def map[A, B](fa: Action[F, R, E, N, A])(
        f: A => B
    ): Action[F, R, E, N, B] = fa.map(f)

    def flatMap[A, B](fa: Action[F, R, E, N, A])(
        f: A => Action[F, R, E, N, B]
    ): Action[F, R, E, N, B] = fa.flatMap(f)

    def tailRecM[A, B](
        a: A
    )(f: A => Action[F, R, E, N, Either[A, B]]): Action[F, R, E, N, B] =
      Action {
        F.tailRecM(a) { a =>
          f(a).run.map { res =>
            val newDecision = res.decision match {
              case e @ Accepted(events @ _, result) =>
                result.map(b => e.copy(result = b))
              case e @ InDecisive(result) => result.map(b => e.copy(result = b))
              case e @ Rejected(reasons)  => Right(Rejected(reasons))
            }

            newDecision.map(ActionResult(_, res.notifications))
          }
        }
      }

    def pure[A](x: A): Action[F, R, E, N, A] =
      Action.pure(x)

}
