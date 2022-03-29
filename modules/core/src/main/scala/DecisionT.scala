package edomata.core

import cats.Applicative
import cats.Functor
import cats.Monad
import cats.data.ValidatedNec
import cats.implicits._
import edomata.core.Decision.Accepted
import edomata.core.Decision.InDecisive
import edomata.core.Decision.Rejected

final case class DecisionT[F[_], R, E, A](run: F[Decision[R, E, A]]) {
  def map[B](f: A => B)(using F: Functor[F]): DecisionT[F, R, E, B] =
    DecisionT(F.map(run)(_.map(f)))

  def flatMap[R2 >: R, E2 >: E, B](
      f: A => DecisionT[F, R2, E2, B]
  )(using M: Monad[F]): DecisionT[F, R2, E2, B] =
    DecisionT {
      M.flatMap(run) {
        case e1 @ Accepted(events1, result) =>
          f(result).run.map {
            case e2 @ Accepted(events2, _) =>
              e2.copy(events = events1 ++ events2)
            case InDecisive(result)     => e1.copy(result = result)
            case r: Rejected[R2, E2, B] => r.copy()
          }
        case InDecisive(result)   => f(result).run
        case r: Rejected[R, E, A] => M.pure(r.copy())
      }
    }

  def as[B](b: B)(using F: Functor[F]): DecisionT[F, R, E, B] = map(_ => b)
}

object DecisionT extends DecisionTConstructors with DecisionTCatsInstances {
  type Of[F[_], R, E] = [t] =>> DecisionT[F, R, E, t]
}

sealed transparent trait DecisionTConstructors {
  def pure[F[_], R, E, T](
      t: T
  )(using F: Applicative[F]): DecisionT[F, R, E, T] =
    DecisionT(F.pure(Decision.pure(t)))

  def void[F[_]: Applicative, R, E]: DecisionT[F, R, E, Unit] = pure(())

  def lift[F[_], R, E, T](
      t: Decision[R, E, T]
  )(using F: Applicative[F]): DecisionT[F, R, E, T] =
    DecisionT(F.pure(t))

  def liftF[F[_], R, E, T](
      f: F[T]
  )(using F: Functor[F]): DecisionT[F, R, E, T] =
    DecisionT(F.map(f)(Decision.pure))

  def validate[F[_]: Applicative, R, E, T](
      validation: ValidatedNec[R, T]
  ): DecisionT[F, R, E, T] =
    lift(Decision.validate(validation))

  def accept[F[_]: Applicative, R, E](
      ev: E,
      evs: E*
  ): DecisionT[F, R, E, Unit] =
    lift(Decision.accept(ev, evs: _*))

  def acceptReturn[F[_]: Applicative, R, E, T](t: T)(
      ev: E,
      evs: E*
  ): DecisionT[F, R, E, T] =
    lift(Decision.acceptReturn(t)(ev, evs: _*))

  def reject[F[_]: Applicative, R, E](
      reason: R,
      otherReasons: R*
  ): DecisionT[F, R, E, Nothing] =
    lift(Decision.reject(reason, otherReasons: _*))
}

sealed transparent trait DecisionTCatsInstances {
  type DT[F[_], R, E] = [T] =>> DecisionT[F, R, E, T]

  given [F[_], R, E](using F: Functor[F]): Functor[DT[F, R, E]] with
    def map[A, B](fa: DecisionT[F, R, E, A])(
        f: A => B
    ): DecisionT[F, R, E, B] = fa.map(f)

  given [F[_], R, E](using F: Monad[F]): Monad[DT[F, R, E]] with
    override def map[A, B](fa: DecisionT[F, R, E, A])(
        f: A => B
    ): DecisionT[F, R, E, B] = fa.map(f)

    def flatMap[A, B](fa: DecisionT[F, R, E, A])(
        f: A => DecisionT[F, R, E, B]
    ): DecisionT[F, R, E, B] = fa.flatMap(f)

    def tailRecM[A, B](
        a: A
    )(f: A => DecisionT[F, R, E, Either[A, B]]): DecisionT[F, R, E, B] =
      DecisionT {
        F.tailRecM(a) { a =>
          f(a).run.map {
            case e @ Accepted(events @ _, result) =>
              result.map(b => e.copy(result = b))
            case e @ InDecisive(result) => result.map(b => e.copy(result = b))
            case e @ Rejected(reasons)  => Right(Rejected(reasons))
          }
        }
      }

    def pure[A](x: A): DecisionT[F, R, E, A] =
      DecisionT.pure(x)

}
