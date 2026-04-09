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

import cats.data.Chain
import cats.data.NonEmptyChain
import cats.effect.IO
import cats.implicits.*
import edomata.backend.eventsourcing.Backend
import edomata.core.*

import java.util.concurrent.CompletableFuture
import scala.jdk.CollectionConverters.*

/** Java-friendly backend facade for event sourcing.
  *
  * Implements AutoCloseable — use with try-with-resources.
  *
  * @tparam S
  *   state type
  * @tparam E
  *   event type
  * @tparam R
  *   rejection type
  * @tparam N
  *   notification type
  */
final class JBackend[S, E, R, N] private[java] (
    private val backend: Backend[IO, S, E, R, N],
    private val model: ModelTC[S, E, R],
    private val runtime: EdomataRuntime,
    private val finalizer: IO[Unit]
) extends AutoCloseable {

  /** Handle a command using the given handler.
    *
    * @return
    *   a future that completes with either rejection reasons or success
    */
  def handle[C](
      handler: JCommandHandler[C, S, E, R, N],
      command: JCommandMessage[C]
  ): CompletableFuture[JEither[java.util.List[R], Unit]] = {
    given ModelTC[S, E, R] = model

    val edomaton: Edomaton[IO, RequestContext[C, S], R, E, N, Unit] =
      Edomaton[IO, RequestContext[C, S], R, E, N, Unit](ctx =>
        IO {
          val jCtx = new JRequestContext(
            ctx.command.payload,
            JCommandMessage.fromScala(ctx.command),
            ctx.state
          )
          val result = handler(jCtx)
          val decision = Converters.decisionToScala(result.decision)
          val notifications =
            Converters.toChain(result.notifications)
          ResponseD(decision, notifications)
        }
      )

    val service = backend.compile(edomaton)

    runtime.unsafeRunAsync(
      service(command.underlying).map {
        case Left(reasons) =>
          JEither.left[java.util.List[R], Unit](reasons.toList.asJava)
        case Right(()) =>
          JEither.right[java.util.List[R], Unit](())
      }
    )
  }

  /** Access the journal reader. */
  def journal: JJournalReader[E] =
    new JJournalReader(backend.journal, runtime)

  /** Access the outbox reader. */
  def outbox: JOutboxReader[N] =
    new JOutboxReader(backend.outbox, runtime)

  /** Release all backend resources. */
  def close(): Unit =
    runtime.unsafeRunAsync(finalizer).join()
}
