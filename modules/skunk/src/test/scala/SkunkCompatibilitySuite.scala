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
import edomata.backend.eventsourcing.Backend
import edomata.skunk.*
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
      backendCross("persistence", jsonbCodec),
      "skunk"
    )

class SkunkPersistenceKeywordNamespaceSuite
    extends PersistenceSuite(
      backend(
        "order",
        jsonbCodec,
        dbName = s"Skunk_${BuildInfo.crossProjectPlatform}"
      ),
      "skunk"
    )

class SkunkCQRSSuite
    extends CqrsSuite(
      backendCqrs("skunk_cqrs", jsonCodec),
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
  private def database(name: String) = Session
    .pooled[IO](
      host = "localhost",
      port = 5432,
      user = "postgres",
      database = name.toLowerCase(),
      password = Some("postgres"),
      4
    )

  inline def backend(
      inline name: String,
      codec: BackendCodec[Int],
      dbName: String = "postgres"
  ): Resource[IO, Backend[IO, Int, Int, String, Int]] =
    given BackendCodec[Int] = codec
    import TestDomain.given_ModelTC_State_Event_Rejection
    database(dbName)
      .flatMap(pool =>
        Backend
          .builder(TestDomainModel)
          .use(SkunkDriver(name, pool))
          // Zero for no buffering in tests
          .persistedSnapshot(maxInMem = 0, maxBuffer = 1)
          .build
      )

  inline def backendCross(
      inline name: String,
      codec: BackendCodec[Int]
  ): Resource[IO, Backend[IO, Int, Int, String, Int]] =
    backend(
      name + "_" + BuildInfo.crossProjectPlatform,
      codec,
      s"Skunk_${BuildInfo.crossProjectPlatform}"
    )

  inline def backendCqrs(
      inline name: String,
      codec: BackendCodec[Int],
      dbName: String = "postgres"
  ): Resource[IO, cqrs.Backend[IO, Int, String, Int]] =
    given BackendCodec[Int] = codec
    import TestCQRSModel.given_StateModelTC_State
    database(dbName)
      .flatMap(pool =>
        Backend
          .builder(TestCQRSDomain)
          .use(SkunkCQRSDriver(name, pool))
          .withRetryConfig(retryInitialDelay = 200.millis)
          .build
      )
}
