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

package edomata.saas

import cats.Id
import cats.data.*
import edomata.core.*
import munit.FunSuite

class ProductCatalogSuite extends FunSuite {

  // --- Domain types (inline, no cross-module dependency) ---

  enum ProductStatus:
    case Draft, Published, Archived

  case class Product(
      name: String,
      description: String,
      priceCents: Long,
      currency: String,
      status: ProductStatus
  )

  enum Cmd:
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

  enum Rejection:
    case NotFound
    case AlreadyExists
    case InvalidTransition(from: String, to: String)
    case InvalidPrice(reason: String)
    case Unauthorized(reason: String)

  enum Notif:
    case Created(name: String, priceCents: Long)
    case DetailsUpdated(name: String)
    case PriceUpdated(priceCents: Long, currency: String)
    case Published
    case Archived
    case Deleted

  // --- Custom auth (API key with scopes) ---

  case class ApiKey(
      tenantId: TenantId,
      ownerId: UserId,
      scopes: Set[String]
  )

  given AuthPolicy[ApiKey] with
    def tenantId(auth: ApiKey): TenantId = auth.tenantId
    def authorize(auth: ApiKey, action: CrudAction): Either[String, Unit] =
      val required = action match
        case CrudAction.Create | CrudAction.Update => "catalog:write"
        case CrudAction.Read                       => "catalog:read"
        case CrudAction.Delete                     => "catalog:admin"
      if auth.scopes(required) then Right(())
      else Left(s"Missing scope: $required")

  // --- Test fixtures ---

  private val tenantA = TenantId("tenant-a")
  private val tenantB = TenantId("tenant-b")
  private val userA = UserId("user-a")

  private val writer =
    ApiKey(tenantA, userA, Set("catalog:read", "catalog:write"))
  private val reader =
    ApiKey(tenantA, userA, Set("catalog:read"))
  private val admin =
    ApiKey(
      tenantA,
      userA,
      Set("catalog:read", "catalog:write", "catalog:admin")
    )
  private val otherTenant =
    ApiKey(
      tenantB,
      userA,
      Set("catalog:read", "catalog:write", "catalog:admin")
    )

  // --- DSL setup ---

  private def mkRejection(msg: String): Rejection =
    if msg.contains("Tenant mismatch") || msg.contains("Entity not found") then
      Rejection.NotFound
    else Rejection.Unauthorized(msg)

  private val dsl =
    SaaSCQRSDomainDSL[ApiKey, Cmd, Product, Rejection, Notif](mkRejection)

  private val draftProduct =
    Product("Widget", "A fine widget", 1999, "USD", ProductStatus.Draft)
  private val publishedProduct =
    draftProduct.copy(status = ProductStatus.Published)
  private val archivedProduct =
    draftProduct.copy(status = ProductStatus.Archived)

  private val activeDraft: CrudState[Product] =
    CrudState.Active(tenantA, userA, draftProduct)
  private val activePublished: CrudState[Product] =
    CrudState.Active(tenantA, userA, publishedProduct)
  private val activeArchived: CrudState[Product] =
    CrudState.Active(tenantA, userA, archivedProduct)
  private val deleted: CrudState[Product] =
    CrudState.Deleted(tenantA, userA)

  private def mkCmd(
      caller: ApiKey,
      payload: Cmd
  ): CommandMessage[SaaSCommand[ApiKey, Cmd]] =
    CommandMessage(
      "cmd-1",
      java.time.Instant.now(),
      "product-1",
      SaaSCommand(caller, payload)
    )

  private def run(
      app: dsl.App[Id, Unit],
      caller: ApiKey,
      state: CrudState[Product],
      payload: Cmd
  ): ResponseE[Rejection, Notif, (CrudState[Product], Unit)] =
    app.run(mkCmd(caller, payload), state)

  // --- Guarded router (the main service logic) ---

  private val serviceApp: dsl.App[Id, Unit] = dsl.guardedRouter[Id] {

    case Cmd.Create(name, desc, price, currency) =>
      (
        CrudAction.Create,
        for
          _ <-
            if price <= 0 then
              dsl.reject[Id, Unit](
                Rejection.InvalidPrice("Price must be positive")
              )
            else dsl.unit[Id]
          c <- dsl.auth[Id]
          _ <- dsl.set[Id](
            CrudState.Active(
              c.tenantId,
              c.ownerId,
              Product(name, desc, price, currency, ProductStatus.Draft)
            )
          )
          _ <- dsl.publish[Id](Notif.Created(name, price))
        yield ()
      )

    case Cmd.UpdateDetails(name, desc) =>
      (
        CrudAction.Update,
        for
          state <- dsl.entityState[Id]
          _ <- state match
            case CrudState.Active(tid, oid, p) =>
              dsl.set[Id](
                CrudState.Active(
                  tid,
                  oid,
                  p.copy(name = name, description = desc)
                )
              ) >> dsl.publish[Id](Notif.DetailsUpdated(name))
            case _ => dsl.reject[Id, Unit](Rejection.NotFound)
        yield ()
      )

    case Cmd.UpdatePrice(price, currency) =>
      (
        CrudAction.Update,
        for
          _ <-
            if price <= 0 then
              dsl.reject[Id, Unit](
                Rejection.InvalidPrice("Price must be positive")
              )
            else dsl.unit[Id]
          state <- dsl.entityState[Id]
          _ <- state match
            case CrudState.Active(tid, oid, p) =>
              dsl.set[Id](
                CrudState.Active(
                  tid,
                  oid,
                  p.copy(priceCents = price, currency = currency)
                )
              ) >> dsl.publish[Id](Notif.PriceUpdated(price, currency))
            case _ => dsl.reject[Id, Unit](Rejection.NotFound)
        yield ()
      )

    case Cmd.Publish =>
      (
        CrudAction.Update,
        for
          state <- dsl.entityState[Id]
          _ <- state match
            case CrudState.Active(tid, oid, p)
                if p.status == ProductStatus.Draft ||
                  p.status == ProductStatus.Archived =>
              dsl.set[Id](
                CrudState.Active(
                  tid,
                  oid,
                  p.copy(status = ProductStatus.Published)
                )
              ) >> dsl.publish[Id](Notif.Published)
            case CrudState.Active(_, _, p) =>
              dsl.reject[Id, Unit](
                Rejection.InvalidTransition(p.status.toString, "Published")
              )
            case _ => dsl.reject[Id, Unit](Rejection.NotFound)
        yield ()
      )

    case Cmd.Archive =>
      (
        CrudAction.Update,
        for
          state <- dsl.entityState[Id]
          _ <- state match
            case CrudState.Active(tid, oid, p)
                if p.status == ProductStatus.Published =>
              dsl.set[Id](
                CrudState.Active(
                  tid,
                  oid,
                  p.copy(status = ProductStatus.Archived)
                )
              ) >> dsl.publish[Id](Notif.Archived)
            case CrudState.Active(_, _, p) =>
              dsl.reject[Id, Unit](
                Rejection.InvalidTransition(p.status.toString, "Archived")
              )
            case _ => dsl.reject[Id, Unit](Rejection.NotFound)
        yield ()
      )

    case Cmd.Delete =>
      (
        CrudAction.Delete,
        for
          state <- dsl.entityState[Id]
          _ <- state match
            case CrudState.Active(tid, oid, p)
                if p.status != ProductStatus.Published =>
              dsl.set[Id](CrudState.Deleted(tid, oid))
                >> dsl.publish[Id](Notif.Deleted)
            case CrudState.Active(_, _, p) =>
              dsl.reject[Id, Unit](
                Rejection.InvalidTransition(p.status.toString, "Deleted")
              )
            case _ => dsl.reject[Id, Unit](Rejection.NotFound)
        yield ()
      )
  }

  // --- Unguarded router (admin bypass) ---

  private val adminApp: dsl.App[Id, Unit] =
    dsl.unsafeUnguardedRouter[Id] {
      case Cmd.Delete =>
        for
          state <- dsl.entityState[Id]
          _ <- state match
            case CrudState.Active(tid, oid, _) =>
              dsl.set[Id](CrudState.Deleted(tid, oid))
                >> dsl.publish[Id](Notif.Deleted)
            case _ => dsl.reject[Id, Unit](Rejection.NotFound)
        yield ()
      case _ =>
        dsl.reject[Id, Unit](Rejection.Unauthorized("Admin: Delete only"))
    }

  // =========================================================================
  // 1. CRUD LIFECYCLE
  // =========================================================================

  test("create: produces Draft state and Created notification") {
    val result = run(
      serviceApp,
      writer,
      CrudState.NonExistent,
      Cmd.Create("Widget", "desc", 1999, "USD")
    )
    assert(result.result.isRight, s"Expected Right, got: ${result.result}")
    result.result match
      case Right((CrudState.Active(_, _, p), ())) =>
        assertEquals(p.status, ProductStatus.Draft)
        assertEquals(p.name, "Widget")
        assertEquals(p.priceCents, 1999L)
      case other => fail(s"Unexpected: $other")
    assertEquals(result.notifications, Chain(Notif.Created("Widget", 1999)))
  }

  test("update details: changes name and description") {
    val result = run(
      serviceApp,
      writer,
      activeDraft,
      Cmd.UpdateDetails("New Name", "New Desc")
    )
    assert(result.result.isRight)
    result.result match
      case Right((CrudState.Active(_, _, p), ())) =>
        assertEquals(p.name, "New Name")
        assertEquals(p.description, "New Desc")
      case other => fail(s"Unexpected: $other")
    assertEquals(result.notifications, Chain(Notif.DetailsUpdated("New Name")))
  }

  test("update price: changes price and currency") {
    val result = run(
      serviceApp,
      writer,
      activeDraft,
      Cmd.UpdatePrice(2499, "EUR")
    )
    assert(result.result.isRight)
    result.result match
      case Right((CrudState.Active(_, _, p), ())) =>
        assertEquals(p.priceCents, 2499L)
        assertEquals(p.currency, "EUR")
      case other => fail(s"Unexpected: $other")
    assertEquals(result.notifications, Chain(Notif.PriceUpdated(2499, "EUR")))
  }

  test("publish: Draft -> Published") {
    val result = run(serviceApp, writer, activeDraft, Cmd.Publish)
    assert(result.result.isRight)
    result.result match
      case Right((CrudState.Active(_, _, p), ())) =>
        assertEquals(p.status, ProductStatus.Published)
      case other => fail(s"Unexpected: $other")
    assertEquals(result.notifications, Chain(Notif.Published))
  }

  test("archive: Published -> Archived") {
    val result = run(serviceApp, writer, activePublished, Cmd.Archive)
    assert(result.result.isRight)
    result.result match
      case Right((CrudState.Active(_, _, p), ())) =>
        assertEquals(p.status, ProductStatus.Archived)
      case other => fail(s"Unexpected: $other")
    assertEquals(result.notifications, Chain(Notif.Archived))
  }

  test("re-publish: Archived -> Published") {
    val result = run(serviceApp, writer, activeArchived, Cmd.Publish)
    assert(result.result.isRight)
    result.result match
      case Right((CrudState.Active(_, _, p), ())) =>
        assertEquals(p.status, ProductStatus.Published)
      case other => fail(s"Unexpected: $other")
  }

  test("delete: Draft -> Deleted") {
    val result = run(serviceApp, admin, activeDraft, Cmd.Delete)
    assert(result.result.isRight)
    result.result match
      case Right((CrudState.Deleted(_, _), ())) => ()
      case other                                => fail(s"Unexpected: $other")
    assertEquals(result.notifications, Chain(Notif.Deleted))
  }

  test("delete: Archived -> Deleted") {
    val result = run(serviceApp, admin, activeArchived, Cmd.Delete)
    assert(result.result.isRight)
    result.result match
      case Right((CrudState.Deleted(_, _), ())) => ()
      case other                                => fail(s"Unexpected: $other")
  }

  // =========================================================================
  // 2. STATE MACHINE VALIDATION
  // =========================================================================

  test("cannot publish already-published product") {
    val result = run(serviceApp, writer, activePublished, Cmd.Publish)
    assert(result.result.isLeft)
    result.result match
      case Left(errs) =>
        assert(
          errs.exists {
            case Rejection.InvalidTransition("Published", "Published") => true
            case _                                                     => false
          },
          s"Expected InvalidTransition, got: $errs"
        )
      case _ => fail("Expected rejection")
  }

  test("cannot archive a draft product") {
    val result = run(serviceApp, writer, activeDraft, Cmd.Archive)
    assert(result.result.isLeft)
    result.result match
      case Left(errs) =>
        assert(
          errs.exists {
            case Rejection.InvalidTransition("Draft", "Archived") => true
            case _                                                => false
          },
          s"Expected InvalidTransition, got: $errs"
        )
      case _ => fail("Expected rejection")
  }

  test("cannot delete a published product") {
    val result = run(serviceApp, admin, activePublished, Cmd.Delete)
    assert(result.result.isLeft)
    result.result match
      case Left(errs) =>
        assert(
          errs.exists {
            case Rejection.InvalidTransition("Published", "Deleted") => true
            case _                                                   => false
          },
          s"Expected InvalidTransition, got: $errs"
        )
      case _ => fail("Expected rejection")
  }

  test("cannot update a deleted product") {
    val result = run(
      serviceApp,
      writer,
      deleted,
      Cmd.UpdateDetails("x", "y")
    )
    assert(result.result.isLeft)
  }

  // =========================================================================
  // 3. CUSTOM AuthPolicy ENFORCEMENT
  // =========================================================================

  test("write scope can create") {
    val result = run(
      serviceApp,
      writer,
      CrudState.NonExistent,
      Cmd.Create("X", "desc", 100, "USD")
    )
    assert(result.result.isRight)
  }

  test("read-only scope cannot create") {
    val result = run(
      serviceApp,
      reader,
      CrudState.NonExistent,
      Cmd.Create("X", "desc", 100, "USD")
    )
    assert(result.result.isLeft)
    result.result match
      case Left(errs) =>
        assert(
          errs.exists {
            case Rejection.Unauthorized(msg) => msg.contains("catalog:write")
            case _                           => false
          },
          s"Expected Unauthorized with scope info, got: $errs"
        )
      case _ => fail("Expected rejection")
  }

  test("read-only scope cannot update") {
    val result = run(
      serviceApp,
      reader,
      activeDraft,
      Cmd.UpdateDetails("x", "y")
    )
    assert(result.result.isLeft)
    result.result match
      case Left(errs) =>
        assert(errs.exists {
          case Rejection.Unauthorized(_) => true
          case _                         => false
        })
      case _ => fail("Expected rejection")
  }

  test("write scope cannot delete (needs catalog:admin)") {
    val result = run(serviceApp, writer, activeDraft, Cmd.Delete)
    assert(result.result.isLeft)
    result.result match
      case Left(errs) =>
        assert(
          errs.exists {
            case Rejection.Unauthorized(msg) => msg.contains("catalog:admin")
            case _                           => false
          },
          s"Expected Unauthorized with admin scope, got: $errs"
        )
      case _ => fail("Expected rejection")
  }

  test("admin scope can delete") {
    val result = run(serviceApp, admin, activeDraft, Cmd.Delete)
    assert(result.result.isRight)
  }

  // =========================================================================
  // 4. MULTI-TENANT ISOLATION
  // =========================================================================

  test("other tenant cannot update product") {
    val result = run(
      serviceApp,
      otherTenant,
      activeDraft,
      Cmd.UpdateDetails("Hacked!", "pwned")
    )
    assert(result.result.isLeft)
    result.result match
      case Left(errs) =>
        assert(
          errs.exists {
            case Rejection.NotFound => true
            case _                  => false
          },
          s"Expected NotFound (tenant mismatch mapped), got: $errs"
        )
      case _ => fail("Expected rejection")
  }

  test("other tenant cannot delete product") {
    val result = run(serviceApp, otherTenant, activeDraft, Cmd.Delete)
    assert(result.result.isLeft)
    result.result match
      case Left(errs) =>
        assert(errs.exists {
          case Rejection.NotFound => true
          case _                  => false
        })
      case _ => fail("Expected rejection")
  }

  // =========================================================================
  // 5. ADMIN BYPASS (unsafeUnguardedRouter)
  // =========================================================================

  test("admin bypass: other tenant can delete via unguarded router") {
    val result = run(adminApp, otherTenant, activeDraft, Cmd.Delete)
    assert(
      result.result.isRight,
      s"Expected Right (bypass), got: ${result.result}"
    )
    result.result match
      case Right((CrudState.Deleted(_, _), ())) => ()
      case other                                => fail(s"Unexpected: $other")
  }

  test("admin bypass: read-only scope can delete via unguarded router") {
    val result = run(adminApp, reader, activeDraft, Cmd.Delete)
    assert(result.result.isRight)
  }

  // =========================================================================
  // 6. NOTIFICATIONS
  // =========================================================================

  test("no notifications on guard rejection") {
    val result = run(
      serviceApp,
      otherTenant,
      activeDraft,
      Cmd.UpdateDetails("x", "y")
    )
    assert(result.result.isLeft)
    assertEquals(result.notifications, Chain.empty[Notif])
  }

  test("no notifications on business logic rejection") {
    val result = run(serviceApp, writer, activePublished, Cmd.Publish)
    assert(result.result.isLeft)
    assertEquals(result.notifications, Chain.empty[Notif])
  }

  // =========================================================================
  // 7. PRICE VALIDATION
  // =========================================================================

  test("create with zero price is rejected") {
    val result = run(
      serviceApp,
      writer,
      CrudState.NonExistent,
      Cmd.Create("X", "desc", 0, "USD")
    )
    assert(result.result.isLeft)
    result.result match
      case Left(errs) =>
        assert(
          errs.exists {
            case Rejection.InvalidPrice(_) => true
            case _                         => false
          },
          s"Expected InvalidPrice, got: $errs"
        )
      case _ => fail("Expected rejection")
  }

  test("create with negative price is rejected") {
    val result = run(
      serviceApp,
      writer,
      CrudState.NonExistent,
      Cmd.Create("X", "desc", -100, "USD")
    )
    assert(result.result.isLeft)
    result.result match
      case Left(errs) =>
        assert(errs.exists {
          case Rejection.InvalidPrice(_) => true
          case _                         => false
        })
      case _ => fail("Expected rejection")
  }

  test("update price to zero is rejected") {
    val result = run(
      serviceApp,
      writer,
      activeDraft,
      Cmd.UpdatePrice(0, "USD")
    )
    assert(result.result.isLeft)
    result.result match
      case Left(errs) =>
        assert(errs.exists {
          case Rejection.InvalidPrice(_) => true
          case _                         => false
        })
      case _ => fail("Expected rejection")
  }
}
