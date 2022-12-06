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

package edomata.skunk

import _root_.skunk.Session
import cats.data.NonEmptyChain
import cats.implicits.*
import cats.Applicative

type SkunkHandler[F[_]] = [N] =>> NonEmptyChain[N] => Session[F] => F[Unit]
object SkunkHandler {
  def apply[F[_]: Applicative, N](
      f: N => Session[F] => F[Unit]
  ): SkunkHandler[F][N] = ns => ses => ns.traverse(f(_)(ses)).void
}
