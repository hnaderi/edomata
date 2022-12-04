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

package edomata.core

import cats.Applicative
import cats.Eval
import cats.Functor
import cats.Monad
import cats.MonadError
import cats.Traverse
import cats.data.Chain
import cats.data.NonEmptyChain
import cats.data.ValidatedNec
import cats.implicits.*
import cats.kernel.Eq
import scala.annotation.tailrec

type ResponseD[R, E, N, A] = ResponseT[Decision[*, E, *], R, N, A]
object ResponseD
    extends ResponseTConstructorsO[
      Decision[*, Nothing, *],
      ResponseD[*, Nothing, *, *]
    ]
