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
import edomata.backend.PGNaming
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

/** Custom authentication context based on API key scopes (NOT CallerIdentity).
  *
  * This demonstrates the pluggable AuthPolicy system — any type can serve as
  * the authentication context as long as an AuthPolicy instance is provided.
  */
final case class ApiKeyContext(
    apiKey: String,
    tenantId: TenantId,
    ownerId: UserId,
    scopes: Set[String]
)

/** Product lifecycle status */
enum ProductStatus derives JsonCodec.AsObject:
  case Draft, Published, Archived

/** Business entity */
final case class Product(
    name: String,
    description: String,
    priceCents: Long,
    currency: String,
    status: ProductStatus
) derives JsonCodec.AsObject

/** Business commands */
enum ProductCommand:
  case Create(
      name: String,
      description: String,
      priceCents: Long,
      currency: String
  )
  case UpdateDetails(name: String, description: String)
  case UpdatePrice(priceCents: Long, currency: String)
  case Publish
  case Archive
  case Delete

/** Notifications emitted to the outbox (integration events) */
enum ProductNotification derives JsonCodec.AsObject:
  case Created(name: String, priceCents: Long)
  case DetailsUpdated(name: String)
  case PriceUpdated(priceCents: Long, currency: String)
  case Published
  case Archived
  case Deleted

/** Typed rejections — guard errors are mapped via mkRejection */
enum ProductRejection derives JsonCodec.AsObject:
  case NotFound
  case AlreadyExists
  case InvalidTransition(from: String, to: String)
  case InvalidPrice(reason: String)
  case Unauthorized(reason: String)

// ---------------------------------------------------------------------------
// 2. CQRS MODEL + AUTH POLICY
// ---------------------------------------------------------------------------

object ProductModel extends CQRSModel[CrudState[Product], ProductRejection]:
  def initial: CrudState[Product] = CrudState.NonExistent

import ProductModel.given

/** Scope-based authorization: catalog:read, catalog:write, catalog:admin */
given AuthPolicy[ApiKeyContext] with
  def tenantId(auth: ApiKeyContext): TenantId = auth.tenantId
  def authorize(
      auth: ApiKeyContext,
      action: CrudAction
  ): Either[String, Unit] =
    val required = action match
      case CrudAction.Create | CrudAction.Update => "catalog:write"
      case CrudAction.Read                       => "catalog:read"
      case CrudAction.Delete                     => "catalog:admin"
    if auth.scopes(required) then Right(())
    else Left(s"Missing scope: $required")

// ---------------------------------------------------------------------------
// 3. SAAS SERVICE (write side — guards are automatic)
// ---------------------------------------------------------------------------

/** Maps guard error strings to typed ProductRejection values */
private def mkRejection(msg: String): ProductRejection =
  if msg.contains("Tenant mismatch") || msg.contains("Entity not found") then
    ProductRejection.NotFound
  else ProductRejection.Unauthorized(msg)

object ProductService
    extends SaaSCQRSService[
      ApiKeyContext,
      ProductCommand,
      Product,
      ProductRejection,
      ProductNotification
    ](mkRejection):
  import SaaS.*

  def apply[F[_]: Monad](): App[F, Unit] = guardedRouter {

    case ProductCommand.Create(name, desc, price, currency) =>
      (
        CrudAction.Create,
        for
          _ <-
            if price <= 0 then
              reject(ProductRejection.InvalidPrice("Price must be positive"))
            else unit
          c <- auth
          _ <- set(
            CrudState.Active(
              c.tenantId,
              c.ownerId,
              Product(name, desc, price, currency, ProductStatus.Draft)
            )
          )
          _ <- publish(ProductNotification.Created(name, price))
        yield ()
      )

    case ProductCommand.UpdateDetails(name, desc) =>
      (
        CrudAction.Update,
        for
          state <- entityState
          _ <- state match
            case CrudState.Active(tid, oid, p) =>
              set(
                CrudState.Active(
                  tid,
                  oid,
                  p.copy(name = name, description = desc)
                )
              )
                >> publish(ProductNotification.DetailsUpdated(name))
            case _ =>
              reject(ProductRejection.NotFound)
        yield ()
      )

    case ProductCommand.UpdatePrice(price, currency) =>
      (
        CrudAction.Update,
        for
          _ <-
            if price <= 0 then
              reject(ProductRejection.InvalidPrice("Price must be positive"))
            else unit
          state <- entityState
          _ <- state match
            case CrudState.Active(tid, oid, p) =>
              set(
                CrudState.Active(
                  tid,
                  oid,
                  p.copy(priceCents = price, currency = currency)
                )
              ) >> publish(ProductNotification.PriceUpdated(price, currency))
            case _ =>
              reject(ProductRejection.NotFound)
        yield ()
      )

    case ProductCommand.Publish =>
      (
        CrudAction.Update,
        for
          state <- entityState
          _ <- state match
            case CrudState.Active(tid, oid, p)
                if p.status == ProductStatus.Draft ||
                  p.status == ProductStatus.Archived =>
              set(
                CrudState.Active(
                  tid,
                  oid,
                  p.copy(status = ProductStatus.Published)
                )
              ) >> publish(ProductNotification.Published)
            case CrudState.Active(_, _, p) =>
              reject(
                ProductRejection.InvalidTransition(
                  p.status.toString,
                  "Published"
                )
              )
            case _ =>
              reject(ProductRejection.NotFound)
        yield ()
      )

    case ProductCommand.Archive =>
      (
        CrudAction.Update,
        for
          state <- entityState
          _ <- state match
            case CrudState.Active(tid, oid, p)
                if p.status == ProductStatus.Published =>
              set(
                CrudState.Active(
                  tid,
                  oid,
                  p.copy(status = ProductStatus.Archived)
                )
              ) >> publish(ProductNotification.Archived)
            case CrudState.Active(_, _, p) =>
              reject(
                ProductRejection.InvalidTransition(
                  p.status.toString,
                  "Archived"
                )
              )
            case _ =>
              reject(ProductRejection.NotFound)
        yield ()
      )

    case ProductCommand.Delete =>
      (
        CrudAction.Delete,
        for
          state <- entityState
          _ <- state match
            case CrudState.Active(tid, oid, p)
                if p.status != ProductStatus.Published =>
              set(CrudState.Deleted(tid, oid))
                >> publish(ProductNotification.Deleted)
            case CrudState.Active(_, _, p) =>
              reject(
                ProductRejection.InvalidTransition(
                  p.status.toString,
                  "Deleted"
                )
              )
            case _ =>
              reject(ProductRejection.NotFound)
        yield ()
      )
  }

// ---------------------------------------------------------------------------
// 4. ADMIN SERVICE (write side — guards bypassed for super-admin)
// ---------------------------------------------------------------------------

object ProductAdminService
    extends SaaSCQRSService[
      ApiKeyContext,
      ProductCommand,
      Product,
      ProductRejection,
      ProductNotification
    ](mkRejection):
  import SaaS.*

  def apply[F[_]: Monad](): App[F, Unit] = unsafeUnguardedRouter {
    case ProductCommand.Delete =>
      for
        state <- entityState
        _ <- state match
          case CrudState.Active(tid, oid, _) =>
            set(CrudState.Deleted(tid, oid))
              >> publish(ProductNotification.Deleted)
          case _ => reject(ProductRejection.NotFound)
      yield ()

    case _ =>
      reject(ProductRejection.Unauthorized("Admin service: Delete only"))
  }

// ---------------------------------------------------------------------------
// 5. SQL READ PROJECTION (via SkunkHandler on notifications)
// ---------------------------------------------------------------------------

/** Read-model projection table DDL:
  * {{{
  * CREATE TABLE IF NOT EXISTS products_read (
  *   id          text        NOT NULL PRIMARY KEY,
  *   tenant_id   text        NOT NULL,
  *   name        text        NOT NULL,
  *   description text        NOT NULL,
  *   price_cents bigint      NOT NULL,
  *   currency    text        NOT NULL,
  *   status      text        NOT NULL,
  *   deleted     boolean     NOT NULL DEFAULT false,
  *   created_at  timestamptz NOT NULL DEFAULT now(),
  *   updated_at  timestamptz NOT NULL DEFAULT now()
  * );
  *
  * -- Mandatory: all tenant-scoped queries filter on this
  * CREATE INDEX IF NOT EXISTS products_read_tenant_idx
  *   ON products_read (tenant_id) WHERE deleted = false;
  *
  * -- Optional: status filtering within a tenant
  * CREATE INDEX IF NOT EXISTS products_read_tenant_status_idx
  *   ON products_read (tenant_id, status) WHERE deleted = false;
  * }}}
  */
object ProductProjection {

  val handler: SkunkHandler[IO][ProductNotification] = SkunkHandler {
    case ProductNotification.Created(name, priceCents) =>
      session =>
        session
          .execute(
            sql"""
              INSERT INTO products_read
                (id, tenant_id, name, description, price_cents, currency, status)
              VALUES ($text, $text, $text, '', $int8, 'USD', 'Draft')
              ON CONFLICT (id) DO UPDATE
              SET name = EXCLUDED.name, price_cents = EXCLUDED.price_cents,
                  status = 'Draft', deleted = false, updated_at = now()
            """.command
          )("placeholder-id", "placeholder-tenant", name, priceCents)
          .void

    case ProductNotification.DetailsUpdated(name) =>
      session =>
        session
          .execute(
            sql"""
              UPDATE products_read
              SET name = $text, updated_at = now()
              WHERE id = $text
            """.command
          )(name, "placeholder-id")
          .void

    case ProductNotification.PriceUpdated(priceCents, currency) =>
      session =>
        session
          .execute(
            sql"""
              UPDATE products_read
              SET price_cents = $int8, currency = $text, updated_at = now()
              WHERE id = $text
            """.command
          )(priceCents, currency, "placeholder-id")
          .void

    case ProductNotification.Published =>
      session =>
        session
          .execute(
            sql"""
              UPDATE products_read SET status = 'Published', updated_at = now()
              WHERE id = $text
            """.command
          )("placeholder-id")
          .void

    case ProductNotification.Archived =>
      session =>
        session
          .execute(
            sql"""
              UPDATE products_read SET status = 'Archived', updated_at = now()
              WHERE id = $text
            """.command
          )("placeholder-id")
          .void

    case ProductNotification.Deleted =>
      session =>
        session
          .execute(
            sql"""
              UPDATE products_read SET deleted = true, updated_at = now()
              WHERE id = $text
            """.command
          )("placeholder-id")
          .void
  }
}

// ---------------------------------------------------------------------------
// 6. SQL READ QUERIES (tenant-scoped by default)
// ---------------------------------------------------------------------------

/** Read model row */
final case class ProductReadModel(
    id: String,
    tenantId: String,
    name: String,
    priceCents: Long,
    currency: String,
    status: String
)

object ProductQueries {

  /** List products for the caller's tenant — tenant filtering is structural */
  val listByTenant
      : TenantScopedQuery[IO, ApiKeyContext, ProductReadModel, Unit] =
    TenantScopedQuery[IO, ApiKeyContext, ProductReadModel, Unit] {
      (tenantId, _) =>
        IO.raiseError(
          new NotImplementedError(
            s"SELECT id, tenant_id, name, price_cents, currency, status " +
              s"FROM products_read WHERE tenant_id = '${tenantId.value}' AND deleted = false"
          )
        )
    }

  /** Admin query — cross-tenant, no tenant filter */
  val adminListAll: UnsafeCrossTenantQuery[IO, ProductReadModel, Unit] =
    UnsafeCrossTenantQuery[IO, ProductReadModel, Unit] { _ =>
      IO.raiseError(
        new NotImplementedError(
          "SELECT id, tenant_id, name, price_cents, currency, status " +
            "FROM products_read WHERE deleted = false"
        )
      )
    }
}

// ---------------------------------------------------------------------------
// 7. WIRING (putting it all together)
// ---------------------------------------------------------------------------

/** Example application showing the full stack with:
  *   - Custom API key auth (not CallerIdentity)
  *   - Typed rejections (not String)
  *   - Product lifecycle state machine
  *   - Cross-tenant isolation demo
  *   - SaaS-aware backend with tenant_id/owner_id columns
  *
  * ===Backend CQRS Tables (generated by SaaSPGSchema)===
  *
  * {{{
  * // Generate DDL for migration tools:
  * SaaSPGSchema.cqrs(PGNaming.prefixed("catalog")).foreach(println)
  *
  * // Output:
  * // CREATE TABLE IF NOT EXISTS catalog_states (
  * //   id text NOT NULL,
  * //   "version" int8 NOT NULL,
  * //   state jsonb NOT NULL,
  * //   tenant_id text NOT NULL,
  * //   owner_id text NOT NULL,
  * //   CONSTRAINT catalog_states_pk PRIMARY KEY (id)
  * // );
  * // CREATE INDEX IF NOT EXISTS catalog_states_tenant_idx ON catalog_states (tenant_id);
  * // CREATE INDEX IF NOT EXISTS catalog_states_tenant_owner_idx ON catalog_states (tenant_id, owner_id);
  * //
  * // CREATE TABLE IF NOT EXISTS catalog_outbox (
  * //   seqnr bigserial NOT NULL,
  * //   stream text NOT NULL,
  * //   correlation text NULL,
  * //   causation text NULL,
  * //   payload jsonb NOT NULL,
  * //   created timestamptz NOT NULL,
  * //   published timestamptz NULL,
  * //   tenant_id text NOT NULL,
  * //   CONSTRAINT catalog_outbox_pk PRIMARY KEY (seqnr)
  * // );
  * // CREATE INDEX IF NOT EXISTS catalog_outbox_tenant_idx ON catalog_outbox (tenant_id);
  * //
  * // CREATE TABLE IF NOT EXISTS catalog_commands (
  * //   id text NOT NULL,
  * //   "time" timestamptz NOT NULL,
  * //   address text NOT NULL,
  * //   CONSTRAINT catalog_commands_pk PRIMARY KEY (id)
  * // );
  *
  * // Optional: enable Row-Level Security
  * // SaaSPGSchema.cqrs(naming, rls = Some(RLSConfig("app_user", "app.tenant_id")))
  * }}}
  */
object ProductCatalogApp extends IOApp.Simple {

  given edomata.skunk.BackendCodec[CrudState[Product]] =
    edomata.skunk.CirceCodec.jsonb
  given edomata.skunk.BackendCodec[ProductNotification] =
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
    CrudState[Product],
    ProductRejection,
    ProductNotification
  ]] =
    for
      driver <- Resource.eval(
        SaaSSkunkCQRSDriver.from[IO](PGNaming.prefixed("catalog"), pool)
      )
      backend <- driver.backend[
        CrudState[Product],
        ProductNotification,
        ProductRejection
      ](handler = Some(ProductProjection.handler))
    yield backend

  // Tenant A: acme-corp with write + read scopes
  val tenantA = ApiKeyContext(
    apiKey = "key-acme-001",
    tenantId = TenantId("acme-corp"),
    ownerId = UserId("alice"),
    scopes = Set("catalog:read", "catalog:write")
  )

  // Tenant B: globex with write + read scopes
  val tenantB = ApiKeyContext(
    apiKey = "key-globex-001",
    tenantId = TenantId("globex"),
    ownerId = UserId("bob"),
    scopes = Set("catalog:read", "catalog:write")
  )

  // Admin: has all scopes including catalog:admin
  val admin = ApiKeyContext(
    apiKey = "key-admin-001",
    tenantId = TenantId("acme-corp"),
    ownerId = UserId("admin"),
    scopes = Set("catalog:read", "catalog:write", "catalog:admin")
  )

  def run: IO[Unit] = database.flatMap(backendRes).use { backend =>
    val service = backend.compile(ProductService[IO]())

    def cmd(
        address: String,
        caller: ApiKeyContext,
        command: ProductCommand
    ) =
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
      // Tenant A creates a product (starts as Draft)
      _ <- cmd(
        "product-1",
        tenantA,
        ProductCommand.Create("Widget", "A fine widget", 1999, "USD")
      ).flatMap(service)
        .flatMap(r => IO.println(s"Create: $r"))

      // Tenant A updates the price
      _ <- cmd("product-1", tenantA, ProductCommand.UpdatePrice(2499, "USD"))
        .flatMap(service)
        .flatMap(r => IO.println(s"Update price: $r"))

      // Tenant A publishes the product
      _ <- cmd("product-1", tenantA, ProductCommand.Publish)
        .flatMap(service)
        .flatMap(r => IO.println(s"Publish: $r"))

      // Tenant B tries to update Tenant A's product — REJECTED (tenant mismatch)
      _ <- cmd(
        "product-1",
        tenantB,
        ProductCommand.UpdateDetails("Hacked!", "pwned")
      ).flatMap(service)
        .flatMap(r => IO.println(s"Cross-tenant attempt: $r"))

      // Cannot delete a published product
      _ <- cmd("product-1", admin, ProductCommand.Delete)
        .flatMap(service)
        .flatMap(r => IO.println(s"Delete published: $r"))

      // Archive first, then delete
      _ <- cmd("product-1", tenantA, ProductCommand.Archive)
        .flatMap(service)
        .flatMap(r => IO.println(s"Archive: $r"))

      _ <- cmd("product-1", admin, ProductCommand.Delete)
        .flatMap(service)
        .flatMap(r => IO.println(s"Delete archived: $r"))

      _ <- IO.println("Done!")
    yield ()
  }
}
