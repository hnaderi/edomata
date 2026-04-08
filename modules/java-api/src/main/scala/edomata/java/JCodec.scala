/*
 * Copyright 2021 Beyond Scale Group
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

package edomata.java

import edomata.doobie.BackendCodec

import java.util.function.{Function => JFunction}

trait JCodec[T] {
  def encode(value: T): String
  def decode(json: String): T
}

object JCodec {
  def of[T](
      encoder: JFunction[T, String],
      decoder: JFunction[String, T]
  ): JCodec[T] = new JCodec[T] {
    def encode(value: T): String = encoder.apply(value)
    def decode(json: String): T = decoder.apply(json)
  }

  private[java] def toBackendCodec[T](codec: JCodec[T]): BackendCodec[T] =
    new BackendCodec.JsonB[T](
      t => codec.encode(t),
      json =>
        try Right(codec.decode(json))
        catch { case e: Exception => Left(e.getMessage) }
    )
}
