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

package crosslang

import cats.effect.ExitCode
import cats.effect.IO
import cats.effect.IOApp
import cats.effect.Resource
import dev.bsg.example.accounts.*
import edomata.backend.Backend
import edomata.backend.PGNamespace
import edomata.backend.eventsourcing.AggregateState
import edomata.core.CommandMessage
import edomata.skunk.BackendCodec
import edomata.skunk.CirceCodec
import edomata.skunk.SkunkDriver
import io.circe.generic.auto.*
import natchez.Trace.Implicits.noop
import skunk.Session
import skunk.codec.all.*
import skunk.implicits.*

import java.time.Instant

/** The Scala side of the Scala/Rust cross-language compatibility test
  * (`rust/crates/edomata-e2e/tests/cross_language.rs`).
  *
  *   - `write <namespace>`: opens the account `xl-account-1`, deposits 100 then
  *     50 through the Skunk backend, and persists the snapshot on shutdown.
  *     Journal, commands, outbox and snapshots are written by Scala.
  *   - `verify <namespace>`: after the Rust side appended a deposit of 25 and a
  *     withdrawal of 5, checks the state, the journal, the outbox, the commands
  *     and the snapshot written by Rust.
  *
  * Connection settings come from `PGHOST`, `PGPORT`, `PGUSER`, `PGPASSWORD` and
  * `PGDATABASE` (docker-compose defaults otherwise).
  */
object CrossLanguage extends IOApp {
  val accountId = "xl-account-1"

  private given BackendCodec[Event] = CirceCodec.jsonb
  private given BackendCodec[Notification] = CirceCodec.jsonb
  private given BackendCodec[Account] = CirceCodec.jsonb

  private def env(name: String, default: String) =
    sys.env.getOrElse(name, default)

  private val pool: Resource[IO, Resource[IO, Session[IO]]] =
    Session.pooled[IO](
      host = env("PGHOST", "localhost"),
      port = env("PGPORT", "5432").toInt,
      user = env("PGUSER", "postgres"),
      password = Some(env("PGPASSWORD", "postgres")),
      database = env("PGDATABASE", "postgres"),
      max = 4
    )

  private def namespace(ns: String): PGNamespace =
    PGNamespace.fromString(ns).fold(sys.error, identity)

  private def backend(ns: String) = for {
    sessions <- pool
    driver = Resource.eval(SkunkDriver.from[IO](namespace(ns), sessions))
    b <- Backend
      .builder(AccountService)
      .from(driver)
      .persistedSnapshot(maxInMem = 100, maxBuffer = 1)
      .withRetryConfig(1)
      .build
  } yield (sessions, b)

  private def cmd(id: String, command: Command) =
    CommandMessage(id, Instant.now(), accountId, command)

  private def check(cond: Boolean, msg: => String): IO[Unit] =
    IO.raiseUnless(cond)(new AssertionError(s"cross-language: $msg"))

  private def write(ns: String): IO[Unit] = backend(ns).use { (_, b) =>
    val service = b.compile(AccountService[IO])
    for {
      r1 <- service(cmd("scala-open", Command.Open))
      r2 <- service(cmd("scala-deposit-100", Command.Deposit(100)))
      r3 <- service(cmd("scala-deposit-50", Command.Deposit(50)))
      _ <- check(
        List(r1, r2, r3).forall(_.isRight),
        s"Scala commands were rejected: $r1 $r2 $r3"
      )
      state <- b.repository.get(accountId)
      _ <- check(
        state == AggregateState.Valid(Account.Open(150), 3),
        s"unexpected state after writing: $state"
      )
      _ <- IO.println(s"[scala] wrote 3 events for $accountId in $ns")
    } yield ()
  }

  private def verify(ns: String): IO[Unit] = backend(ns).use { (sessions, b) =>
    for {
      state <- b.repository.get(accountId)
      _ <- check(
        state == AggregateState.Valid(Account.Open(170), 5),
        s"unexpected state: $state"
      )
      events <- b.journal.readStream(accountId).map(_.payload).compile.toList
      _ <- check(
        events == List(
          Event.Opened,
          Event.Deposited(100),
          Event.Deposited(50),
          Event.Deposited(25),
          Event.Withdrawn(5)
        ),
        s"unexpected journal: $events"
      )
      versions <- b.journal
        .readStream(accountId)
        .map(_.metadata.version)
        .compile
        .toList
      _ <- check(
        versions == List(0L, 1L, 2L, 3L, 4L),
        s"unexpected versions: $versions"
      )
      outbox <- b.outbox.read.map(_.data).compile.toList
      _ <- check(
        outbox == List(
          Notification.AccountOpened(accountId),
          Notification.BalanceUpdated(accountId, 100),
          Notification.BalanceUpdated(accountId, 150),
          Notification.BalanceUpdated(accountId, 175),
          Notification.BalanceUpdated(accountId, 170)
        ),
        s"unexpected outbox: $outbox"
      )
      commands <- sessions.use(
        _.unique(sql"""SELECT count(*) FROM "#$ns".commands""".query(int8))
      )
      _ <- check(commands == 5L, s"unexpected command count: $commands")
      snapshot <- sessions.use(
        _.unique(
          sql"""SELECT "version" FROM "#$ns".snapshots WHERE id = $text"""
            .query(int8)
        )(accountId)
      )
      _ <- check(snapshot == 5L, s"unexpected snapshot version: $snapshot")
      _ <- IO.println(
        s"[scala] verified $accountId in $ns: balance 170, version 5, 5 events, 5 notifications, 5 commands, snapshot 5"
      )
    } yield ()
  }

  def run(args: List[String]): IO[ExitCode] = args match {
    case "write" :: ns :: Nil  => write(ns).as(ExitCode.Success)
    case "verify" :: ns :: Nil => verify(ns).as(ExitCode.Success)
    case _                     =>
      IO.println("usage: CrossLanguage (write|verify) <namespace>")
        .as(ExitCode.Error)
  }
}
