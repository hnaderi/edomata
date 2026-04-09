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
import cats.effect.kernel.Resource
import cats.effect.unsafe.IORuntime
import doobie.Transactor
import edomata.backend.Backend
import edomata.backend.PGNamespace
import edomata.backend.PGNaming
import edomata.core.*
import edomata.doobie.BackendCodec
import edomata.doobie.DoobieDriver

import javax.sql.DataSource

/** Java-friendly builder for the Doobie event sourcing backend. */
final class JBackendBuilder[S, E, R, N] private (
    private val domainModel: JDomainModel[S, E, R],
    private val namingOpt: Option[PGNaming],
    private val dataSourceOpt: Option[DataSource],
    private val eventCodecOpt: Option[BackendCodec[E]],
    private val notificationCodecOpt: Option[BackendCodec[N]],
    private val maxRetry: Int,
    private val inMemSnapshotSize: Int,
    private val skipSetup: Boolean
) {
  private def copy(
      namingOpt: Option[PGNaming] = this.namingOpt,
      dataSourceOpt: Option[DataSource] = this.dataSourceOpt,
      eventCodecOpt: Option[BackendCodec[E]] = this.eventCodecOpt,
      notificationCodecOpt: Option[BackendCodec[N]] = this.notificationCodecOpt,
      maxRetry: Int = this.maxRetry,
      inMemSnapshotSize: Int = this.inMemSnapshotSize,
      skipSetup: Boolean = this.skipSetup
  ): JBackendBuilder[S, E, R, N] =
    new JBackendBuilder(
      domainModel,
      namingOpt,
      dataSourceOpt,
      eventCodecOpt,
      notificationCodecOpt,
      maxRetry,
      inMemSnapshotSize,
      skipSetup
    )

  /** Set the PostgreSQL namespace (uses prefixed naming strategy). */
  def namespace(ns: String): JBackendBuilder[S, E, R, N] = {
    val pgns = PGNamespace
      .fromString(ns)
      .fold(
        err => throw new IllegalArgumentException(s"Invalid namespace: $err"),
        identity
      )
    copy(namingOpt = Some(PGNaming.Prefixed(pgns)))
  }

  /** Set the PostgreSQL namespace with schema naming strategy. */
  def schemaNamespace(ns: String): JBackendBuilder[S, E, R, N] = {
    val pgns = PGNamespace
      .fromString(ns)
      .fold(
        err => throw new IllegalArgumentException(s"Invalid namespace: $err"),
        identity
      )
    copy(namingOpt = Some(PGNaming.Schema(pgns)))
  }

  /** Set the JDBC DataSource. */
  def dataSource(ds: DataSource): JBackendBuilder[S, E, R, N] =
    copy(dataSourceOpt = Some(ds))

  /** Set the event codec using a JCodec. */
  def eventCodec(codec: JCodec[E]): JBackendBuilder[S, E, R, N] =
    copy(eventCodecOpt = Some(JCodec.toBackendCodec(codec)))

  /** Set the notification codec using a JCodec. */
  def notificationCodec(codec: JCodec[N]): JBackendBuilder[S, E, R, N] =
    copy(notificationCodecOpt = Some(JCodec.toBackendCodec(codec)))

  /** Set the maximum number of retries on version conflict. */
  def maxRetry(n: Int): JBackendBuilder[S, E, R, N] =
    copy(maxRetry = n)

  /** Set the in-memory snapshot cache size. */
  def inMemSnapshotSize(size: Int): JBackendBuilder[S, E, R, N] =
    copy(inMemSnapshotSize = size)

  /** Skip automatic DDL setup (for Flyway/manual migrations). */
  def skipSetup(skip: Boolean): JBackendBuilder[S, E, R, N] =
    copy(skipSetup = skip)

  /** Build the backend. This allocates resources (DB connections, caches).
    *
    * The returned JBackend implements AutoCloseable — use try-with-resources.
    *
    * @throws IllegalStateException
    *   if required configuration is missing
    */
  def build(runtime: EdomataRuntime): JBackend[S, E, R, N] = {
    val naming = namingOpt.getOrElse(
      throw new IllegalStateException("namespace is required")
    )
    val ds = dataSourceOpt.getOrElse(
      throw new IllegalStateException("dataSource is required")
    )
    val eventCodec = eventCodecOpt.getOrElse(
      throw new IllegalStateException("eventCodec is required")
    )
    val notifCodec = notificationCodecOpt.getOrElse(
      throw new IllegalStateException("notificationCodec is required")
    )

    given ModelTC[S, E, R] = domainModel.toModelTC
    given BackendCodec[E] = eventCodec
    given BackendCodec[N] = notifCodec

    val transactor = Transactor
      .fromDataSource[IO](ds, scala.concurrent.ExecutionContext.global)
    val domain = Domain[Nothing, S, E, R, N]()

    val resource
        : Resource[IO, edomata.backend.eventsourcing.Backend[IO, S, E, R, N]] =
      Resource
        .eval(DoobieDriver.from[IO](naming, transactor, skipSetup))
        .flatMap { driver =>
          Backend
            .builder(domain)
            .use(driver)
            .inMemSnapshot(inMemSnapshotSize)
            .withRetryConfig(maxRetry = maxRetry)
            .build
        }

    val (backend, finalizer) =
      resource.allocated.unsafeRunSync()(using runtime.runtime)

    new JBackend(backend, domainModel.toModelTC, runtime, finalizer)
  }
}

object JBackendBuilder {

  /** Create a builder for the Doobie event sourcing backend. */
  def forDoobie[S, E, R, N](
      model: JDomainModel[S, E, R]
  ): JBackendBuilder[S, E, R, N] =
    new JBackendBuilder(
      domainModel = model,
      namingOpt = None,
      dataSourceOpt = None,
      eventCodecOpt = None,
      notificationCodecOpt = None,
      maxRetry = 5,
      inMemSnapshotSize = 1000,
      skipSetup = false
    )
}
