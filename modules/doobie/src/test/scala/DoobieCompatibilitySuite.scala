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

import _root_.doobie.util.transactor.Transactor
import cats.effect.IO
import cats.effect.kernel.Resource
import cats.implicits.*
import edomata.backend.*
import edomata.doobie.*
import scodec.bits.ByteVector

import DoobieCompatibilitySuite.*

class DoobieJsonCompatibilitySuite
    extends BackendCompatibilitySuite(
      backend("compatibility_json", jsonCodec),
      "doobie json"
    )
class DoobieJsonbCompatibilitySuite
    extends BackendCompatibilitySuite(
      backend("compatibility_jsonb", jsonbCodec),
      "doobie jsonb"
    )
class DoobieBinaryCompatibilitySuite
    extends BackendCompatibilitySuite(
      backend("compatibility_binary", binCodec),
      "doobie binary"
    )

class DoobiePersistenceSuite
    extends PersistenceSuite(
      backend("doobie_persistence", jsonbCodec),
      "doobie"
    )

class DoobiePersistenceKeywordNamespaceSuite
    extends PersistenceSuite(
      backend("order", jsonbCodec),
      "doobie"
    )

class DoobieCQRSSuite
    extends CqrsSuite(
      backendCqrs("doobie_cqrs", jsonCodec),
      "doobie"
    )

object DoobieCompatibilitySuite {
  val jsonCodec: BackendCodec.Json[Int] =
    BackendCodec.Json(_.toString, _.toIntOption.toRight("Not a number"))
  val jsonbCodec: BackendCodec.JsonB[Int] =
    BackendCodec.JsonB(_.toString, _.toIntOption.toRight("Not a number"))
  val binCodec: BackendCodec.Binary[Int] = BackendCodec.Binary(
    i => ByteVector.fromHex(i.toString).get.toArray,
    a => Either.catchNonFatal(ByteVector(a).toHex.toInt).leftMap(_.getMessage)
  )
  private val trx = Transactor
    .fromDriverManager[IO](
      driver = "org.postgresql.Driver",
      url = "jdbc:postgresql:postgres",
      user = "postgres",
      password = "postgres",
      logHandler = None
    )

  inline def backend(
      inline name: String,
      codec: BackendCodec[Int]
  ): Resource[IO, Backend[IO, Int, Int, String, Int]] =
    given BackendCodec[Int] = codec
    import TestDomain.given_ModelTC_State_Event_Rejection

    Backend
      .builder(TestDomainModel)
      .use(DoobieDriver(name, trx))
      // Zero for no buffering in tests
      .persistedSnapshot(maxInMem = 0, maxBuffer = 1)
      .build

  inline def backendCqrs(
      inline name: String,
      codec: BackendCodec[Int]
  ): Resource[IO, cqrs.Backend[IO, Int, String, Int]] =
    given BackendCodec[Int] = codec
    import TestCQRSModel.given_StateModelTC_State

    Backend
      .builder(TestCQRSDomain)
      .use(DoobieCQRSDriver(name, trx))
      .build
}
