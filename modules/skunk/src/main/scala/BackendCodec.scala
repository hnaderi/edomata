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

import skunk.Codec
import skunk.codec.binary.bytea
import skunk.data.Type

import scala.annotation.implicitNotFound

@implicitNotFound(
  "Cannot find how to handle serialization/deserialization for ${T}"
)
sealed trait BackendCodec[T] {
  def oid: Type
  def codec: Codec[T]
}

object BackendCodec {
  @implicitNotFound("Cannot find a way to build json codec for ${T}")
  final class Json[T](encode: T => String, decode: String => Either[String, T])
      extends BackendCodec[T] {
    final def oid = Type.json
    def codec: Codec[T] = Codec.simple(encode, decode, oid)
  }

  @implicitNotFound("Cannot find a way to build jsonb codec for ${T}")
  final class JsonB[T](encode: T => String, decode: String => Either[String, T])
      extends BackendCodec[T] {
    final def oid = Type.jsonb
    def codec: Codec[T] = Codec.simple(encode, decode, oid)
  }

  @implicitNotFound("Cannot find a way to build binary codec for ${T}")
  final class Binary[T](
      encode: T => Array[Byte],
      decode: Array[Byte] => Either[String, T]
  ) extends BackendCodec[T] {
    def oid: Type = Type.bytea
    def codec: Codec[T] = bytea.eimap(decode)(encode)
  }
}
