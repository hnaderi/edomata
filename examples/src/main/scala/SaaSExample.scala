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

package edomata.examples.saas

import cats.Monad

import cats.effect.IO
import cats.effect.IOApp
import cats.effect.kernel.Resource
import cats.implicits.*
import edomata.backend.Backend
import edomata.core.*
import edomata.saas.*
import edomata.skunk.*
import io.circe.Codec as JsonCodec
import io.circe.Decoder
import io.circe.Encoder
import io.circe.generic.auto.*
import skunk.Session
import skunk.*
import skunk.codec.all.*
import skunk.implicits.*

import natchez.Trace.Implicits.noop

// ---------------------------------------------------------------------------
// 1. DOMAIN TYPES
// ---------------------------------------------------------------------------

/** Business entity */
final case class Todo(title: String, completed: Boolean)
    derives JsonCodec.AsObject

/** Circe codecs for opaque types and CrudState */
given Encoder[TenantId] = Encoder.encodeString.contramap(_.value)
given Decoder[TenantId] = Decoder.decodeString.map(TenantId(_))
given Encoder[UserId] = Encoder.encodeString.contramap(_.value)
given Decoder[UserId] = Decoder.decodeString.map(UserId(_))
given [A: Encoder: Decoder]: JsonCodec.AsObject[CrudState[A]] =
  JsonCodec.AsObject.derived

/** Business commands */
enum TodoCommand:
  case Create(title: String)
  case Complete
  case Delete

/** Notifications emitted to the outbox (integration events) */
enum TodoNotification derives JsonCodec.AsObject:
  case Created(entityId: String, tenantId: String, title: String)
  case Completed(entityId: String, tenantId: String)
  case Deleted(entityId: String, tenantId: String)

// ---------------------------------------------------------------------------
// 2. CQRS MODEL (state = CrudState[Todo])
// ---------------------------------------------------------------------------

object TodoModel extends CQRSModel[CrudState[Todo], String]:
  def initial: CrudState[Todo] = CrudState.NonExistent

// Make the StateModelTC available in scope
import TodoModel.given

// Role-based auth policy for this service
given AuthPolicy[CallerIdentity] = RoleBasedPolicy {
  case CrudAction.Create => Set("todo:write")
  case CrudAction.Read   => Set("todo:read")
  case CrudAction.Update => Set("todo:write")
  case CrudAction.Delete => Set("todo:admin")
}

// ---------------------------------------------------------------------------
// 3. SAAS SERVICE (write side -- guards are automatic)
// ---------------------------------------------------------------------------

object TodoService
    extends SaaSCQRSService[
      CallerIdentity,
      TodoCommand,
      Todo,
      String,
      TodoNotification
    ](
      mkRejection = identity
    ):
  import SaaS.*

  def apply[F[_]: Monad](): App[F, Unit] = guardedRouter {

    case TodoCommand.Create(title) =>
      (
        CrudAction.Create,
        for
          c <- auth
          id <- aggregateId
          _ <- set(CrudState.Active(c.tenantId, c.userId, Todo(title, false)))
          _ <- publish(
            TodoNotification.Created(id, c.tenantId.value, title)
          )
        yield ()
      )

    case TodoCommand.Complete =>
      (
        CrudAction.Update,
        for
          id <- aggregateId
          state <- entityState
          _ <- state match
            case CrudState.Active(tid, oid, todo) =>
              set(CrudState.Active(tid, oid, todo.copy(completed = true)))
                >> publish(TodoNotification.Completed(id, tid.value))
            case _ => reject("Todo not found")
        yield ()
      )

    case TodoCommand.Delete =>
      (
        CrudAction.Delete,
        for
          id <- aggregateId
          state <- entityState
          _ <- state match
            case CrudState.Active(tid, oid, _) =>
              set(CrudState.Deleted(tid, oid))
                >> publish(TodoNotification.Deleted(id, tid.value))
            case _ => reject("Todo not found")
        yield ()
      )
  }

// ---------------------------------------------------------------------------
// 4. ADMIN SERVICE (write side -- guards bypassed for super-admin)
// ---------------------------------------------------------------------------

object TodoAdminService
    extends SaaSCQRSService[
      CallerIdentity,
      TodoCommand,
      Todo,
      String,
      TodoNotification
    ](
      mkRejection = identity
    ):
  import SaaS.*

  def apply[F[_]: Monad](): App[F, Unit] = unsafeUnguardedRouter {
    // Super-admin can delete any todo across any tenant
    case TodoCommand.Delete =>
      for
        state <- entityState
        _ <- state match
          case CrudState.Active(tid, oid, _) =>
            set(CrudState.Deleted(tid, oid))
          case _ => reject("Todo not found")
      yield ()

    case _ => reject("Admin service only supports Delete")
  }

// ---------------------------------------------------------------------------
// 5. SQL READ PROJECTION (via SkunkHandler on notifications)
// ---------------------------------------------------------------------------

/** This handler is called for each notification in the outbox. It maintains a
  * denormalized read-model table optimized for queries.
  *
  * Read-model table DDL:
  * {{{
  * CREATE TABLE todos_read (
  *   id         text        NOT NULL,
  *   tenant_id  text        NOT NULL,
  *   title      text        NOT NULL,
  *   completed  boolean     NOT NULL DEFAULT false,
  *   deleted    boolean     NOT NULL DEFAULT false,
  *   PRIMARY KEY (id)
  * );
  * CREATE INDEX todos_read_tenant_idx ON todos_read (tenant_id);
  * }}}
  */
object TodoProjection {

  val handler: SkunkHandler[IO][TodoNotification] = SkunkHandler {
    case TodoNotification.Created(entityId, tenantId, title) =>
      session =>
        session
          .execute(
            sql"""
              INSERT INTO todos_read (id, tenant_id, title, completed, deleted)
              VALUES ($text, $text, $text, false, false)
              ON CONFLICT (id) DO UPDATE
              SET title = EXCLUDED.title, completed = false, deleted = false
            """.command
          )(entityId, tenantId, title)
          .void

    case TodoNotification.Completed(entityId, _) =>
      session =>
        session
          .execute(
            sql"""
              UPDATE todos_read SET completed = true WHERE id = $text
            """.command
          )(entityId)
          .void

    case TodoNotification.Deleted(entityId, _) =>
      session =>
        session
          .execute(
            sql"""
              UPDATE todos_read SET deleted = true WHERE id = $text
            """.command
          )(entityId)
          .void
  }
}

// ---------------------------------------------------------------------------
// 6. SQL READ QUERIES (tenant-scoped by default)
// ---------------------------------------------------------------------------

/** Read model row */
final case class TodoReadModel(
    id: String,
    tenantId: String,
    title: String,
    completed: Boolean
)

object TodoQueries {

  /** List todos for the caller's tenant -- tenant filtering is structural */
  val listByTenant: TenantScopedQuery[IO, CallerIdentity, TodoReadModel, Unit] =
    TenantScopedQuery[IO, CallerIdentity, TodoReadModel, Unit] {
      (tenantId, _) =>
        // In a real app, you'd use a Skunk session from a pool.
        // This is a placeholder showing the query shape.
        IO.raiseError(
          new NotImplementedError(
            s"SELECT id, tenant_id, title, completed FROM todos_read WHERE tenant_id = '${tenantId.value}' AND deleted = false"
          )
        )
    }

  /** Admin query -- cross-tenant, no tenant filter */
  val adminListAll: UnsafeCrossTenantQuery[IO, TodoReadModel, Unit] =
    UnsafeCrossTenantQuery[IO, TodoReadModel, Unit] { _ =>
      IO.raiseError(
        new NotImplementedError(
          "SELECT id, tenant_id, title, completed FROM todos_read WHERE deleted = false"
        )
      )
    }
}

// ---------------------------------------------------------------------------
// 7. WIRING (putting it all together)
// ---------------------------------------------------------------------------

/** Example application showing the full stack */
object SaaSApp extends IOApp.Simple {

  given edomata.skunk.BackendCodec[CrudState[Todo]] =
    edomata.skunk.CirceCodec.jsonb
  given edomata.skunk.BackendCodec[TodoNotification] =
    edomata.skunk.CirceCodec.jsonb

  val database: Resource[IO, Resource[IO, Session[IO]]] = Session
    .pooled[IO](
      host = "localhost",
      port = 5432,
      user = "postgres",
      password = Some("postgres"),
      database = "postgres",
      max = 10
    )

  def backendRes(
      pool: Resource[IO, Session[IO]]
  ): Resource[IO, edomata.backend.cqrs.Backend[
    IO,
    CrudState[Todo],
    String,
    TodoNotification
  ]] =
    for
      driver <- Resource.eval(
        SkunkCQRSDriver[IO]("todos", pool)
      )
      backend <- Backend
        .builder(TodoService.domain)
        .use(driver)
        .withEventHandler(TodoProjection.handler)
        .build
    yield backend

  val tenantA = CallerIdentity(
    TenantId("acme-corp"),
    UserId("alice"),
    Set("todo:write", "todo:read")
  )
  val tenantB = CallerIdentity(
    TenantId("globex"),
    UserId("bob"),
    Set("todo:write", "todo:read")
  )

  def run: IO[Unit] = database.flatMap(backendRes).use { backend =>
    val service = backend.compile(TodoService[IO]())

    def cmd(address: String, caller: CallerIdentity, command: TodoCommand) =
      IO.randomUUID.flatMap { id =>
        IO.realTime
          .map { d => java.time.Instant.ofEpochMilli(d.toMillis) }
          .map { now =>
            CommandMessage(
              id.toString,
              now,
              address,
              SaaSCommand(caller, command)
            )
          }
      }

    for
      // Tenant A creates a todo
      _ <- cmd("todo-1", tenantA, TodoCommand.Create("Buy milk"))
        .flatMap(service)
        .flatMap(r => IO.println(s"Create: $r"))

      // Tenant A completes it
      _ <- cmd("todo-1", tenantA, TodoCommand.Complete)
        .flatMap(service)
        .flatMap(r => IO.println(s"Complete: $r"))

      // Tenant B tries to modify Tenant A's todo -- REJECTED (tenant mismatch)
      _ <- cmd("todo-1", tenantB, TodoCommand.Complete)
        .flatMap(service)
        .flatMap(r => IO.println(s"Cross-tenant attempt: $r"))

      // Read: tenant-scoped query would look like this:
      // val todos = TodoQueries.listByTenant.query(tenantA, ())
      _ <- IO.println("Done!")
    yield ()
  }
}
