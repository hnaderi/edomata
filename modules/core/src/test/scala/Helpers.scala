/*
 * Copyright 2021 Hossein Naderi
 *
 * Licensed under the Apache License, Version 2.0 (the "License");
 * you may not use this file except in compliance with the License.
 * You may obtain a copy of the License at
 *
 *     http://www.apache.org/licenses/LICENSE-2.0
 *
 * Unless required by applicable law or agreed to in writing, software
 * distributed under the License is distributed on an "AS IS" BASIS,
 * WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
 * See the License for the specific language governing permissions and
 * limitations under the License.
 */

package tests

import cats.Functor
import cats.Monad
import cats.data.NonEmptyChain
import cats.implicits.*
import org.scalacheck.Arbitrary
import org.scalacheck.Gen
import org.scalacheck.Prop.forAll

private[tests] def necOf[T](g: Gen[T]): Gen[NonEmptyChain[T]] =
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

private[tests] given [T: Arbitrary]: Gen[NonEmptyChain[T]] = necOf(
  Arbitrary.arbitrary[T]
)

private[tests] given [F[_]: Functor]: Arbitrary[F[Long] => F[Long]] = Arbitrary(
  Arbitrary.arbitrary[Long].map(i => (a: F[Long]) => a.map(_ + i))
)
