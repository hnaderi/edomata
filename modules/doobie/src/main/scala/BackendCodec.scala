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
import cats.Show
import cats.data.NonEmptyList
import cats.implicits.*
import edomata.backend.*
import org.postgresql.util.PGobject

import scala.annotation.implicitNotFound

@implicitNotFound(
  "Cannot find how to handle serialization/deserialization for ${T}"
)
sealed trait BackendCodec[T] {
  def tpe: String
  def codec: Meta[T]
}

object BackendCodec {
  private given Show[PGobject] = Show.show(_.getValue.take(250))
  private given Show[Array[Byte]] = Show.show(_.toString)

  private val pgJsonGet: Get[PGobject] =
    Get.Advanced.other[PGobject](NonEmptyList.of("json"))
  private val pgJsonPut: Put[PGobject] =
    Put.Advanced.other[PGobject](NonEmptyList.of("json"))

  private val pgJsonBGet: Get[PGobject] =
    Get.Advanced.other[PGobject](NonEmptyList.of("jsonb"))
  private val pgJsonBPut: Put[PGobject] =
    Put.Advanced.other[PGobject](NonEmptyList.of("jsonb"))

  sealed abstract class JsonBase[T](
      put: Put[PGobject],
      get: Get[PGobject],
      val tpe: String
  )(
      encode: T => String,
      decode: String => Either[String, T]
  ) extends BackendCodec[T] {
    final def codec: Meta[T] =
      new Meta(
        get.temap(o => decode(o.getValue)),
        put.tcontramap[T] { t =>
          val o = new PGobject
          o.setType(tpe)
          o.setValue(encode(t))

          o
        }
      )

  }

  @implicitNotFound("Cannot find a way to build json codec for ${T}")
  final class Json[T](encode: T => String, decode: String => Either[String, T])
      extends JsonBase[T](pgJsonPut, pgJsonGet, "json")(encode, decode)

  @implicitNotFound("Cannot find a way to build jsonb codec for ${T}")
  final class JsonB[T](encode: T => String, decode: String => Either[String, T])
      extends JsonBase[T](pgJsonBPut, pgJsonBGet, "jsonb")(encode, decode)

  @implicitNotFound("Cannot find a way to build binary codec for ${T}")
  final class Binary[T](
      encode: T => Array[Byte],
      decode: Array[Byte] => Either[String, T]
  ) extends BackendCodec[T] {
    def tpe = "bytea"
    def codec: Meta[T] = new Meta(
      Meta.ByteArrayMeta.get.temap(decode),
      Meta.ByteArrayMeta.put.tcontramap(encode)
    )
  }
}
