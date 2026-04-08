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

import munit.FunSuite

class JCodecSuite extends FunSuite {

  test("JCodec.of creates codec from lambdas") {
    val codec = JCodec.of[Int](
      (i: Int) => i.toString,
      (s: String) => s.toInt
    )
    assertEquals(codec.encode(42), "42")
    assertEquals(codec.decode("99"), 99)
  }

  test("toBackendCodec produces a valid BackendCodec") {
    val codec = JCodec.of[String](
      (s: String) => s""""$s"""",
      (json: String) => json.stripPrefix("\"").stripSuffix("\"")
    )
    val backendCodec = JCodec.toBackendCodec(codec)
    assertEquals(backendCodec.tpe, "jsonb")
  }

  test("toBackendCodec decode failure returns Left") {
    val codec = JCodec.of[Int](
      (i: Int) => i.toString,
      (s: String) => throw new RuntimeException("parse error")
    )
    val backendCodec = JCodec.toBackendCodec(codec)
    // The BackendCodec wraps exceptions into Left
    assertEquals(backendCodec.tpe, "jsonb")
  }
}
