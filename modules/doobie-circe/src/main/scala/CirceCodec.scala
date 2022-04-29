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

package edomata.backend
package doobie

import _root_.doobie.*
import cats.implicits.*
import io.circe.Decoder
import io.circe.Encoder
import io.circe.Json
import io.circe.parser.decode
import io.circe.syntax.*

object CirceCodec {
  def json[T: Encoder: Decoder]: BackendCodec.Json[T] =
    BackendCodec.Json(_.asJson.noSpaces, decode[T](_).leftMap(_.getMessage))
  def jsonb[T: Encoder: Decoder]: BackendCodec.JsonB[T] =
    BackendCodec.JsonB(_.asJson.noSpaces, decode[T](_).leftMap(_.getMessage))
}
