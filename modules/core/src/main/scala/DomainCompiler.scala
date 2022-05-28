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

package edomata.core

import cats.Applicative
import cats.Monad
import cats.data.Chain
import cats.data.EitherNec
import cats.data.NonEmptyChain
import cats.implicits.*

import java.time.OffsetDateTime
import java.time.ZoneOffset
import scala.util.NotGiven

type DomainService[F[_], C, R] = C => F[EitherNec[R, Unit]]

object DomainCompiler {
  def execute[F[_]: Monad, C, S, E, R, N](
      app: Edomaton[F, RequestContext[C, S], R, E, N, Unit],
      ctx: RequestContext[C, S]
  )(using m: ModelTC[S, E, R]): F[EdomatonResult[S, E, R, N]] =
    app.run(ctx).map { case Response(decision, notifs) =>
      m.perform(ctx.state, decision) match {
        case Decision.Accepted(evs, newState) =>
          EdomatonResult.Accepted(newState, evs, notifs)
        case Decision.InDecisive(_) =>
          EdomatonResult.Indecisive(notifs)
        case Decision.Rejected(errs) if decision.isRejected =>
          EdomatonResult.Rejected(notifs, errs)
        case Decision.Rejected(errs) =>
          EdomatonResult.Conflicted(errs)
      }
    }
}

/** Representation of the result of running an edomaton */
enum EdomatonResult[S, E, R, N] {
  case Accepted(
      newState: S,
      events: NonEmptyChain[E],
      notifications: Chain[N]
  )
  case Indecisive(
      notifications: Chain[N]
  )
  case Rejected(
      notifications: Chain[N],
      reasons: NonEmptyChain[R]
  )
  case Conflicted(
      reasons: NonEmptyChain[R]
  )
}

private[edomata] transparent trait EdomatonSyntax {
  extension [F[_]: Monad, C, S, E, R, N, T](
      app: Edomaton[F, RequestContext[C, S], R, E, N, T]
  )(using NotGiven[T =:= Unit], ModelTC[S, E, R]) {

    inline def execute(
        ctx: RequestContext[C, S]
    ): F[EdomatonResult[S, E, R, N]] =
      DomainCompiler.execute(app.void, ctx)
  }

  extension [F[_]: Monad, C, S, E, R, N](
      app: Edomaton[F, RequestContext[C, S], R, E, N, Unit]
  )(using m: ModelTC[S, E, R]) {
    inline def execute(
        ctx: RequestContext[C, S]
    ): F[EdomatonResult[S, E, R, N]] = DomainCompiler.execute(app, ctx)
  }

  private def fk[F[_]: Applicative]: cats.arrow.FunctionK[cats.Id, F] = new {
    def apply[A](a: A): F[A] = Applicative[F].pure(a)
  }

  extension (app: Edomaton[cats.Id, ?, ?, ?, ?, ?]) {
    def liftTo[F[_]](using F: Applicative[F]) = app.mapK(fk[F])
  }
}
