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

import cats.effect.IO
import cats.effect.unsafe.IORuntime

/** Manages the Cats Effect IORuntime lifecycle.
  *
  * This should typically be a singleton per application. Implements
  * AutoCloseable for use with try-with-resources.
  */
final class EdomataRuntime private (
    private[java] val runtime: IORuntime,
    private val ownsRuntime: Boolean
) extends AutoCloseable {

  /** Shuts down the runtime, releasing all threads. */
  def close(): Unit =
    if (ownsRuntime) runtime.shutdown()

  private[java] def unsafeRunAsync[A](
      io: IO[A]
  ): java.util.concurrent.CompletableFuture[A] =
    io.unsafeToCompletableFuture()(using runtime)
}

object EdomataRuntime {

  /** Creates a new runtime with default settings. */
  def create(): EdomataRuntime =
    new EdomataRuntime(
      cats.effect.unsafe.IORuntime.global,
      ownsRuntime = false
    )

  /** Creates a new runtime backed by IORuntime.global. This is the recommended
    * approach — it shares the global compute pool and does not need explicit
    * shutdown.
    */
  def global(): EdomataRuntime =
    new EdomataRuntime(
      cats.effect.unsafe.IORuntime.global,
      ownsRuntime = false
    )

  /** Wraps an existing IORuntime. The caller retains ownership — close() will
    * NOT shut it down.
    */
  def fromExisting(runtime: IORuntime): EdomataRuntime =
    new EdomataRuntime(runtime, ownsRuntime = false)
}
