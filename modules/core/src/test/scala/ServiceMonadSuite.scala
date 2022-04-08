package edomata.core

import cats.Eval
import cats.Monad
import cats.data.NonEmptyChain
import cats.implicits.*
import cats.kernel.laws.discipline.EqTests
import cats.laws.discipline.*
import cats.laws.discipline.arbitrary.*
import cats.laws.discipline.eq.*
import munit.*
import org.scalacheck.Arbitrary
import org.scalacheck.Gen
import org.scalacheck.Prop.forAll

class ServiceMonadSuite extends DisciplineSuite {

  private given [Env, T: Arbitrary]: Arbitrary[AppG[Env, T]] = Arbitrary(
    for {
      n <- ResponseSuite.notifications
      t <- Arbitrary.arbitrary[T]
      d <- Generators.anySut
    } yield ServiceMonad(_ => Some(Response(d.as(t), n)))
  )

  private given ExhaustiveCheck[Int] = ExhaustiveCheck.instance(List(1, 2, 3))

  checkAll(
    "laws",
    MonadTests[App].monad[Int, Long, String]
  )
  checkAll(
    "laws",
    ContravariantTests[AppContra].contravariant[Int, Long, Boolean]
  )
  checkAll("laws", EqTests[App[Long]].eqv)
}
