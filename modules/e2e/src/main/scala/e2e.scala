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

import cats.effect.IO
import cats.effect.Resource
import dev.hnaderi.example.accounts.*
import edomata.backend.Backend
import edomata.backend.eventsourcing
import edomata.backend.eventsourcing.AggregateState
import edomata.backend.eventsourcing.StorageDriver
import edomata.core.CommandMessage
import munit.CatsEffectSuite
import munit.Location

import java.time.Instant

abstract class e2e[Codec[_]](driver: Resource[IO, StorageDriver[IO, Codec]])(
    using
    Codec[Account],
    Codec[Event],
    Codec[Notification]
) extends CatsEffectSuite {

  private final case class SUT(
      app: eventsourcing.Backend[IO, Account, Event, Rejection, Notification]
  ) {
    val service = app.compile(AccountService[IO])

    def open(address: String) = cmd(address, Command.Open).flatMap(service)
    def deposit(address: String, amount: BigDecimal) =
      cmd(address, Command.Deposit(amount)).flatMap(service)

    def state(
        address: String
    ) =
      app.repository
        .get(address)

    def assertState(
        address: String,
        balance: BigDecimal,
        version: Long
    )(using Location) =
      state(address)
        .assertEquals(
          AggregateState.Valid(Account.Open(balance), version)
        )
  }

  private val AppWithCache = Backend
    .builder(AccountService)
    .from(driver)
    .persistedSnapshot(maxInMem = 100)
    .withRetryConfig(0)
    .build
    .map(SUT(_))

  private val AppNoCache = Backend
    .builder(AccountService)
    .from(driver)
    .disableCache
    .persistedSnapshot(maxInMem = 100)
    .withRetryConfig(0)
    .build
    .map(SUT(_))

  def randomString = IO.randomUUID.map(_.toString())

  def cmd(address: String, cmd: Command) = for {
    newID <- randomString
    now <- IO.realTime.map(fd => Instant.ofEpochMilli(fd.toMillis))
  } yield CommandMessage(
    newID.toString(),
    now,
    address,
    cmd
  )

  test("Sanity") {
    AppNoCache.use(app =>
      for {
        address <- randomString
        _ <- app.open(address)
        _ <- app.assertState(address, 0, 1)
      } yield ()
    )
  }

  test("Distributed workload should work without cache") {
    Resource
      .both(AppNoCache, AppNoCache)
      .use((appA, appB) =>
        for {
          address <- tests.randomString
          _ <- appA.open(address)
          _ <- appA.deposit(address, 100)
          _ <- appB.deposit(address, 50)

          _ <- appA.assertState(address, 150, 3)
          _ <- appB.assertState(address, 150, 3)
        } yield ()
      )
  }

  test("Distributed workload doesn't work with default cache") {
    Resource
      .both(AppWithCache, AppWithCache)
      .use((appA, appB) =>
        for {
          address <- tests.randomString
          _ <- appA.open(address)
          _ <- appA.deposit(address, 100)

          // B doesn't contain entity yet, so no problem
          _ <- appB.deposit(address, 50)

          _ <- appA.assertState(address, 150, 3)
          // Now, both the applications have the same entity cached in memory
          // If one of the apps changes that entity, it will make the other application's cache incorrect
          // So the other application won't be able to issue a command on that entity anymore due to version conflicts
          _ <- appB.assertState(address, 150, 3)

          _ <- appA.deposit(address, 100).attempt
          _ <- appB.deposit(address, 50)

          _ <- appA.assertState(address, 200, 4)
          _ <- appB.assertState(address, 200, 4)
        } yield ()
      )
  }

}
