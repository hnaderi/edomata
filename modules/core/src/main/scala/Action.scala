package edomata.core

import cats.Applicative
import cats.Functor
import cats.Monad
import cats.data.Chain
import cats.data.NonEmptyChain
import cats.data.ValidatedNec
import cats.implicits._
import cats.kernel.Eq

final case class Action[F[_], R, E, N, A](run: F[Response[R, E, N, A]]) {
  def map[B](f: A => B)(using F: Functor[F]): Action[F, R, E, N, B] =
    Action(F.map(run)(_.map(f)))

  def flatMap[R2 >: R, E2 >: E, N2 >: N, B](
      f: A => Action[F, R2, E2, N2, B]
  )(using M: Monad[F]): Action[F, R2, E2, N2, B] =
    Action {
      M.flatMap(run)(a =>
        a.result.visit(
          err => a.copy(result = Decision.Rejected(err)).pure,
          aa => f(aa).run.map(a >> _)
        )
      )
    }

  def as[B](b: B)(using F: Functor[F]): Action[F, R, E, N, B] = map(_ => b)

  /** Clears all notifications so far */
  def reset(using F: Functor[F]): Action[F, R, E, N, A] =
    Action(run.map(_.reset))

  /** Adds notification without considering decision state */
  def publish(ns: N*)(using F: Functor[F]): Action[F, R, E, N, A] =
    Action(
      run.map(_.publish(ns: _*))
    )
}

object Action extends ActionConstructors with ActionCatsInstances

sealed transparent trait ActionConstructors {
  def lift[F[_], R, E, N, T](
      t: Response[R, E, N, T]
  )(using F: Applicative[F]): Action[F, R, E, N, T] =
    Action(F.pure(t))

  def liftD[F[_], R, E, N, T](
      t: Decision[R, E, T]
  )(using F: Applicative[F]): Action[F, R, E, N, T] =
    lift(Response(t))

  def pure[F[_], R, E, N, T](
      t: T
  )(using F: Applicative[F]): Action[F, R, E, N, T] =
    lift(Response(Decision.pure(t)))

  def void[F[_]: Applicative, R, E, N]: Action[F, R, E, N, Unit] = pure(())

  def liftF[F[_], R, E, N, T](
      f: F[T]
  )(using F: Functor[F]): Action[F, R, E, N, T] =
    Action(F.map(f)(d => Response(Decision.pure(d))))

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
    lift(Response(Decision.unit, Chain.fromSeq(ns)))
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
        F.tailRecM((a, Chain.empty[N], Chain.empty[E])) { (a, ns0, evs0) =>
          f(a).run.map { res =>
            res.result match {
              case Decision.Accepted(evs, eab) =>
                eab.fold(
                  a =>
                    (a, ns0 ++ res.notifications, evs0 ++ evs.toChain).asLeft,
                  b =>
                    Response(
                      Decision.Accepted(evs.prependChain(evs0), b),
                      ns0 ++ res.notifications
                    ).asRight
                )
              case Decision.InDecisive(eab) =>
                eab.fold(
                  a => (a, ns0 ++ res.notifications, evs0).asLeft,
                  b =>
                    NonEmptyChain
                      .fromChain(evs0)
                      .fold(
                        Response(
                          Decision.InDecisive(b),
                          ns0 ++ res.notifications
                        )
                      )(evs =>
                        Response(
                          Decision.Accepted(evs, b),
                          ns0 ++ res.notifications
                        )
                      )
                      .asRight
                )
              case Decision.Rejected(err) =>
                res.copy(result = Decision.Rejected(err)).pure
            }

          }
        }
      }

    def pure[A](x: A): Action[F, R, E, N, A] =
      Action.pure(x)

  given [F[_], R, E, N, A](using
      Eq[F[Response[R, E, N, A]]]
  ): Eq[Action[F, R, E, N, A]] = Eq.by(_.run)
}
