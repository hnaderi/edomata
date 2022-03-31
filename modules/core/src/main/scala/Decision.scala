package edomata.core

import cats.MonadError
import cats.data.NonEmptyChain
import cats.data.Validated
import cats.data.ValidatedNec
import cats.data.EitherNec

import scala.annotation.tailrec

import Decision._
import cats.data.Chain

/** Represents states in a decision context */
enum Decision[R, E, +T] {
  case InDecisive(result: T)
  case Accepted(events: NonEmptyChain[E], result: T)
  case Rejected(reasons: NonEmptyChain[R])
}

object Decision extends DecisionConstructors, DecisionCatsInstances, DecisionOps

sealed trait DecisionConstructors {
  def pure[R, E, T](t: T): Decision[R, E, T] = InDecisive(t)

  def unit[R, E]: Decision[R, E, Unit] = InDecisive(())

  def accept[R, E](ev: E, evs: E*): Decision[R, E, Unit] =
    acceptReturn(())(ev, evs: _*)

  def acceptReturn[R, E, T](t: T)(ev: E, evs: E*): Decision[R, E, T] =
    Accepted(NonEmptyChain.of(ev, evs: _*), t)

  def reject[R, E](
      reason: R,
      otherReasons: R*
  ): Decision[R, E, Nothing] =
    Rejected(NonEmptyChain.of(reason, otherReasons: _*))

  def validate[R, E, T](
      validation: ValidatedNec[R, T]
  ): Decision[R, E, T] =
    validation match {
      case Validated.Invalid(e) => Rejected(e)
      case Validated.Valid(a)   => InDecisive(a)
    }
}

trait DecisionOps {
  extension [R, E, A](self: Decision[R, E, A]) {
    def map[B](f: A => B): Decision[R, E, B] =
      self match {
        case e: InDecisive[R, E, A] => e.copy(result = f(e.result))
        case e: Accepted[R, E, A]   => e.copy(result = f(e.result))
        case Rejected(reasons)      => Rejected(reasons)
      }

    def flatMap[R2 >: R, E2 >: E, B](
        f: A => Decision[R2, E2, B]
    ): Decision[R2, E2, B] =
      self match {
        case Accepted(events, result) =>
          f(result) match {
            case Accepted(events2, result) =>
              Accepted(events ++ events2, result)
            case InDecisive(result) => Accepted(events, result)
            case other              => other
          }
        case InDecisive(result) => f(result)
        case Rejected(reasons)  => Rejected(reasons)
      }

    def isRejected: Boolean = self match {
      case Rejected(_) => true
      case _           => false
    }

    def isAccepted: Boolean = self match {
      case Accepted(_, _) => true
      case _              => false
    }

    def fold[B](fr: NonEmptyChain[R] => B, fa: A => B): B = self match {
      case Decision.InDecisive(a)  => fa(a)
      case Decision.Accepted(_, a) => fa(a)
      case Decision.Rejected(r)    => fr(r)
    }

    def toValidated: ValidatedNec[R, A] = self match {
      case Decision.Accepted(_, t) => Validated.Valid(t)
      case Decision.InDecisive(t)  => Validated.Valid(t)
      case Decision.Rejected(r)    => Validated.Invalid(r)
    }
    def toOption: Option[A] = fold(_ => None, Some(_))
    def toEither: EitherNec[R, A] = fold(Left(_), Right(_))
  }

  extension [R, T](self: ValidatedNec[R, T]) {
    def toDecision[E]: Decision[R, E, T] = self match {
      case Validated.Valid(t)   => Decision.InDecisive(t)
      case Validated.Invalid(r) => Decision.Rejected(r)
    }
  }
}

type D[R, E] = [T] =>> Decision[R, E, T]

sealed trait DecisionCatsInstances {
  given [R, E]: MonadError[D[R, E], NonEmptyChain[R]] =
    new MonadError[D[R, E], NonEmptyChain[R]] {
      override def pure[A](x: A): Decision[R, E, A] = Decision.pure(x)

      override def map[A, B](
          fa: Decision[R, E, A]
      )(f: A => B): Decision[R, E, B] =
        fa.map(f)

      override def flatMap[A, B](fa: Decision[R, E, A])(
          f: A => Decision[R, E, B]
      ): Decision[R, E, B] =
        fa.flatMap(f)

      @tailrec
      private def step[A, B](a: A, evs: Chain[E] = Chain.empty)(
          f: A => Decision[R, E, Either[A, B]]
      ): Decision[R, E, B] =
        f(a) match {
          case Decision.Accepted(ev2, Left(a)) =>
            step(a, evs ++ ev2.toChain)(f)
          case Decision.Accepted(ev2, Right(b)) =>
            Decision.Accepted(ev2.prependChain(evs), b)
          case Decision.InDecisive(Left(a)) => step(a, evs)(f)
          case Decision.InDecisive(Right(b)) =>
            NonEmptyChain
              .fromChain(evs)
              .fold(Decision.InDecisive(b))(Decision.Accepted(_, b))
          case Decision.Rejected(rs) => Decision.Rejected(rs)
        }

      override def tailRecM[A, B](a: A)(
          f: A => Decision[R, E, Either[A, B]]
      ): Decision[R, E, B] =
        step(a)(f)

      def handleErrorWith[A](
          fa: Decision[R, E, A]
      )(f: NonEmptyChain[R] => Decision[R, E, A]): Decision[R, E, A] =
        fa match {
          case Decision.Rejected(e) => f(e)
          case other                => other
        }

      def raiseError[A](e: NonEmptyChain[R]): Decision[R, E, A] =
        Decision.Rejected(e)

    }
}
