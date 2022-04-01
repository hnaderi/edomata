package edomata.core

import cats.Applicative
import cats.Functor
import cats.Monad
import cats.MonadError
import cats.data.Chain
import cats.data.NonEmptyChain
import cats.data.ValidatedNec
import cats.implicits.*

import scala.annotation.tailrec
import cats.kernel.Eq

final case class ResponseMonad[R, E, N, +A](
    result: Decision[R, E, A],
    notifications: Seq[N] = Nil
) {
  def map[B](f: A => B): ResponseMonad[R, E, N, B] =
    copy(result = result.map(f))

  def flatMap[N2 >: N, B](
      f: A => ResponseMonad[R, E, N2, B]
  ): ResponseMonad[R, E, N2, B] = result.fold(
    rej => ResponseMonad(Decision.Rejected(rej), notifications),
    a => {
      val out = f(a)
      (result >> out.result) match {
        case d @ Decision.Rejected(_) => ResponseMonad(d, out.notifications)
        case other => ResponseMonad(other, notifications ++ out.notifications)
      }
    }
  )

  def as[B](b: B): ResponseMonad[R, E, N, B] = map(_ => b)

  /** Clears all notifications so far */
  def reset: ResponseMonad[R, E, N, A] =
    copy(notifications = Nil)

  /** Adds notification without considering decision state */
  def publish(ns: N*): ResponseMonad[R, E, N, A] =
    copy(notifications = notifications ++ ns)

  def publishOnRejectionWith(
      f: NonEmptyChain[R] => Seq[N]
  ): ResponseMonad[R, E, N, A] = publish(result.fold(f, _ => Nil): _*)
  def publishOnRejection(ns: N*): ResponseMonad[R, E, N, A] =
    publishOnRejectionWith(_ => ns)
}

object ResponseMonad extends ResponseMonadConstructors {
  given [R, E, N]: Monad[[t] =>> ResponseMonad[R, E, N, t]] =
    new Monad {
      override def map[A, B](fa: ResponseMonad[R, E, N, A])(
          f: A => B
      ): ResponseMonad[R, E, N, B] = fa.map(f)

      def flatMap[A, B](fa: ResponseMonad[R, E, N, A])(
          f: A => ResponseMonad[R, E, N, B]
      ): ResponseMonad[R, E, N, B] = fa.flatMap(f)

      @tailrec
      private def step[A, B](
          a: A,
          evs: Chain[E] = Chain.empty,
          ns: Seq[N] = Nil
      )(
          f: A => ResponseMonad[R, E, N, Either[A, B]]
      ): ResponseMonad[R, E, N, B] =
        val out = f(a)
        out.result match {
          case Decision.Accepted(ev2, e) =>
            e match {
              case Left(a) =>
                step(a, evs ++ ev2.toChain, ns ++ out.notifications)(f)
              case Right(b) =>
                ResponseMonad(
                  Decision.Accepted(ev2.prependChain(evs), b),
                  ns ++ out.notifications
                )
            }
          case Decision.InDecisive(e) =>
            e match {
              case Left(a) =>
                step(a, evs, ns ++ out.notifications)(f)
              case Right(b) =>
                ResponseMonad(
                  NonEmptyChain
                    .fromChain(evs)
                    .fold(Decision.InDecisive(b))(Decision.Accepted(_, b)),
                  ns ++ out.notifications
                )
            }
          case Decision.Rejected(rs) =>
            out.copy(result = Decision.Rejected(rs))
        }

      def tailRecM[A, B](
          a: A
      )(
          f: A => ResponseMonad[R, E, N, Either[A, B]]
      ): ResponseMonad[R, E, N, B] =
        step(a)(f)

      def pure[A](x: A): ResponseMonad[R, E, N, A] =
        ResponseMonad(Decision.pure(x))
    }

  given [R, E, N, T]: Eq[ResponseMonad[R, E, N, T]] = Eq.instance(_ == _)
}

sealed trait ResponseMonadConstructors {
  def pure[R, E, N, T](t: T): ResponseMonad[R, E, N, T] = ResponseMonad(
    Decision.pure(t)
  )

  def unit[R, E, N]: ResponseMonad[R, E, N, Unit] = pure(())

  def lift[R, E, N, T](d: Decision[R, E, T]): ResponseMonad[R, E, N, T] =
    ResponseMonad(d)

  def publish[R, E, N](n: N*): ResponseMonad[R, E, N, Unit] =
    ResponseMonad(Decision.unit, n)

  def accept[R, E, N](ev: E, evs: E*): ResponseMonad[R, E, N, Unit] =
    acceptReturn(())(ev, evs: _*)

  def acceptReturn[R, E, N, T](
      t: T
  )(ev: E, evs: E*): ResponseMonad[R, E, N, T] =
    ResponseMonad(Decision.Accepted(NonEmptyChain.of(ev, evs: _*), t))

  def reject[R, E, N](
      reason: R,
      otherReasons: R*
  ): ResponseMonad[R, E, N, Nothing] =
    ResponseMonad(Decision.Rejected(NonEmptyChain.of(reason, otherReasons: _*)))

  def validate[R, E, N, T](
      validation: ValidatedNec[R, T]
  ): ResponseMonad[R, E, N, T] =
    ResponseMonad(Decision.validate(validation))
}
