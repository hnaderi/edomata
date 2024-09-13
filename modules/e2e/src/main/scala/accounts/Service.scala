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

package dev.hnaderi.example.accounts

enum Command {
  case Open
  case Deposit(amount: BigDecimal)
  case Withdraw(amount: BigDecimal)
  case Close
}

enum Notification {
  case AccountOpened(accountId: String)
  case BalanceUpdated(accountId: String, balance: BigDecimal)
  case AccountClosed(accountId: String)
}

object AccountService extends Account.Service[Command, Notification] {
  import cats.Monad

  def apply[F[_]: Monad]: App[F, Unit] = App.router {

    case Command.Open =>
      for {
        ns <- App.state.decide(_.open)
        acc <- App.aggregateId
        _ <- App.publish(Notification.AccountOpened(acc))
      } yield ()

    case Command.Deposit(amount) =>
      for {
        deposited <- App.state.decide(_.deposit(amount))
        accId <- App.aggregateId
        _ <- App.publish(Notification.BalanceUpdated(accId, deposited.balance))
      } yield ()

    case Command.Withdraw(amount) =>
      for {
        withdrawn <- App.state.decide(_.withdraw(amount))
        accId <- App.aggregateId
        _ <- App.publish(Notification.BalanceUpdated(accId, withdrawn.balance))
      } yield ()

    case Command.Close =>
      App.state.decide(_.close).void
  }

}
