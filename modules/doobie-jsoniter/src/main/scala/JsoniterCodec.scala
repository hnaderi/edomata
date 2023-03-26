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

package edomata.doobie

import _root_.doobie.*
import cats.implicits.*
import com.github.plokhotnyuk.jsoniter_scala.core.*
import BackendCodec.*

object JsoniterCodec {
  def json[T: JsonValueCodec]: BackendCodec.Json[T] =
    BackendCodec.Json(
      writeToString(_),
      s => Either.catchNonFatal(readFromString[T](s)).leftMap(_.getMessage())
    )

  def jsonb[T: JsonValueCodec]: BackendCodec.JsonB[T] =
    BackendCodec.JsonB(
      writeToString(_),
      s => Either.catchNonFatal(readFromString[T](s)).leftMap(_.getMessage())
    )

  def msgpack[T: JsonValueCodec]: Binary[T] =
    BackendCodec.Binary[T](
      writeToArray(_),
      i => Either.catchNonFatal(readFromArray(i)).leftMap(_.getMessage)
    )
}
