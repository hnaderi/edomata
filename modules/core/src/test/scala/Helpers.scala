package edomata.core

import cats.Functor
import cats.Monad
import cats.data.NonEmptyChain
import cats.implicits.*
import org.scalacheck.Arbitrary
import org.scalacheck.Gen
import org.scalacheck.Prop.forAll

private[core] def necOf[T](g: Gen[T]): Gen[NonEmptyChain[T]] =
  Gen
    .chooseNum(1, 10)
    .flatMap(n =>
      Gen
        .listOfN(n, g)
        .map(NonEmptyChain.fromSeq)
        .flatMap {
          case Some(e) => Gen.const(e)
          case None    => Gen.fail
        }
    )

private[core] given [F[_]: Functor]: Arbitrary[F[Long] => F[Long]] = Arbitrary(
  Arbitrary.arbitrary[Long].map(i => (a: F[Long]) => a.map(_ + i))
)
