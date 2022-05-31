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

package edomata.munit

import cats.Monad
import cats.data.*
import cats.implicits.*
import cats.kernel.Order
import edomata.core.*
import edomata.syntax.all.*
import munit.FunSuite
import munit.Location

import java.time.Instant

trait DomainSuite(msgId: String = "1", address: String = "sut")
    extends FunSuite {
  extension [F[_], C, S, E, R, N](
      self: Edomaton[F, RequestContext[C, S], R, E, N, Unit]
  )(using Monad[F], ModelTC[S, E, R], Location) {
    def runWith(command: C, state: S): F[EdomatonResult[S, E, R, N]] =
      self.execute(
        RequestContext(
          CommandMessage(id = msgId, Instant.MIN, address = address, command),
          state
        )
      )

    /** Asserts exactly state and notifications
      */
    def expect(command: C, state: S)(
        expectedState: S,
        expectedNotifications: N*
    ): F[Unit] = self.runWith(command, state).map {
      case EdomatonResult.Accepted(ns, _, notifs) =>
        assertEquals(ns, expectedState)
        assertEquals(notifs, Chain.fromSeq(expectedNotifications))
      case other => fail(s"Expected success, but obtained: $other")
    }

    /** Asserts exactly state, and contains all notifications in any order
      */
    def expectAll(command: C, state: S)(
        expectedState: S,
        expectedNotifications: N*
    )(using Order[N]): F[Unit] = self.runWith(command, state).map {
      case EdomatonResult.Accepted(ns, _, notifs) =>
        assertEquals(ns, expectedState)
        assertEquals(notifs.sorted, Chain.fromSeq(expectedNotifications.sorted))
      case other => fail(s"Expected success, but obtained: $other")
    }

    def expectRejection(
        command: C,
        state: S
    ): F[EdomatonResult.Rejected[S, E, R, N]] =
      self.runWith(command, state).map {
        case r: EdomatonResult.Rejected[S, E, R, N] => r
        case other => fail(s"Expected rejection, but obtained: $other")
      }

    def expectRejectionWith(command: C, state: S)(
        expectedErrors: NonEmptyChain[R],
        expectedNotifications: Chain[N] = Chain.empty
    ): F[Unit] =
      self.runWith(command, state).map {
        case EdomatonResult.Rejected(notifs, errs) =>
          assertEquals(errs, expectedErrors)
          assertEquals(notifs, expectedNotifications)
        case other => fail(s"Expected rejection, but obtained: $other")
      }

    def expectRejectionWith(command: C, state: S)(
        err1: R,
        errs: R*
    ): F[Unit] =
      expectRejectionWith(command, state)(
        NonEmptyChain.of(err1, errs: _*),
        Chain.empty[N]
      )

    def expectRejectionNotify(command: C, state: S)(
        notifs: N*
    ): F[Unit] =
      self.runWith(command, state).map {
        case EdomatonResult.Rejected(ns, _) =>
          assertEquals(ns, Chain.fromSeq(notifs))
        case other => fail(s"Expected rejection, but obtained: $other")
      }

    def expectThat(command: C, state: S)(
        expectedNotifications: N*
    )(that: PartialFunction[S, Unit]): F[Unit] =
      self.runWith(command, state).map {
        case EdomatonResult.Accepted(ns, _, notifs) =>
          assertEquals(notifs, Chain.fromSeq(expectedNotifications))
          that.applyOrElse(ns, _ => ())
        case other => fail(s"Expected success, but obtained: $other")
      }
  }
}
