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

import _root_.skunk.Session
import cats.effect.IO
import cats.effect.kernel.Resource
import cats.implicits.*
import edomata.backend.*
import edomata.core.DomainModel
import edomata.skunk.*
import edomata.syntax.all.*
import natchez.Trace.Implicits.noop
import scodec.bits.ByteVector
import scala.concurrent.duration.*

import SkunkCompatibilitySuite.*

class SkunkCompatibilitySuite
    extends BackendCompatibilitySuite(
      backend("compatibility_json", jsonCodec),
      "skunk json"
    )
class SkunkJsonbCompatibilitySuite
    extends BackendCompatibilitySuite(
      backend("compatibility_jsonb", jsonbCodec),
      "skunk jsonb"
    )
class SkunkBinaryCompatibilitySuite
    extends BackendCompatibilitySuite(
      backend("compatibility_binary", binCodec),
      "skunk binary"
    )

class SkunkPersistenceSuite
    extends PersistenceSuite(
      backend("skunk_persistence", jsonbCodec),
      "skunk"
    )

class SkunkPersistenceKeywordNamespaceSuite
    extends PersistenceSuite(
      backend("order", jsonbCodec),
      "skunk"
    )

class SkunkCQRSSuite
    extends CqrsSuite(
      backendCqrs("cqrs_test", jsonCodec),
      "skunk"
    )

object SkunkCompatibilitySuite {
  val jsonCodec: BackendCodec.Json[Int] =
    BackendCodec.Json(_.toString, _.toIntOption.toRight("Not a number"))
  val jsonbCodec: BackendCodec.JsonB[Int] =
    BackendCodec.JsonB(_.toString, _.toIntOption.toRight("Not a number"))
  val binCodec: BackendCodec.Binary[Int] = BackendCodec.Binary(
    i => ByteVector.fromHex(i.toString).get.toArray,
    a => Either.catchNonFatal(ByteVector(a).toHex.toInt).leftMap(_.getMessage)
  )
  private val database = Session
    .pooled[IO](
      "localhost",
      5432,
      "postgres",
      "postgres",
      Some("postgres"),
      4
    )

  inline def backend(
      inline name: String,
      codec: BackendCodec[Int]
  ): Resource[IO, Backend[IO, Int, Int, String, Int]] =
    given BackendCodec[Int] = codec
    import TestDomain.given_ModelTC_State_Event_Rejection
    database
      .flatMap(pool =>
        Backend
          .builder(TestDomainModel)
          .use(SkunkDriver(name, pool))
          // Zero for no buffering in tests
          .persistedSnapshot(maxInMem = 0, maxBuffer = 1)
          .build
      )

  inline def backendCqrs(
      inline name: String,
      codec: BackendCodec[Int]
  ): Resource[IO, cqrs.Backend[IO, Int, String, Int]] =
    given BackendCodec[Int] = codec
    import TestCQRSModel.given_StateModelTC_State
    database
      .flatMap(pool =>
        Backend
          .builder(TestCQRSDomain)
          .use(SkunkDriverCQRS(name, pool))
          .withRetryConfig(retryInitialDelay = 100.millis)
          .build
      )
}
