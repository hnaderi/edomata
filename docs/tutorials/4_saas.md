# SaaS Multi-Tenant Module

## Overview

The `edomata-saas` module provides generic multi-tenant CRUD abstractions that automatically enforce **tenant isolation** and **authorization** on both reads and writes. When you extend the SaaS service traits, every command is guarded before business logic runs -- no manual checks needed.

Authorization is **pluggable**: you define your own auth type (JWT claims, API key context, session token, etc.) and implement the `AuthPolicy` typeclass. The module ships with `CallerIdentity` and `RoleBasedPolicy` as a convenient default.

## Getting Started

Add the SaaS module to your project. Do **not** add `edomata-core` directly -- it comes transitively:

```scala
libraryDependencies += "dev.hnaderi" %% "edomata-saas" % "@VERSION@"
```

Import everything from `edomata.saas`:

```scala
import edomata.saas.*
```

## Core Types

### AuthPolicy (typeclass)

The central abstraction for authentication and authorization. Implement this for your auth context type:

```scala
trait AuthPolicy[Auth]:
  /** Extract the tenant identifier from the auth context */
  def tenantId(auth: Auth): TenantId

  /** Verify authorization for the given action.
    * Return Right(()) if allowed, Left(reason) if denied. */
  def authorize(auth: Auth, action: CrudAction): Either[String, Unit]
```

### CallerIdentity (default auth type)

A built-in auth context with role-based access control:

```scala
final case class CallerIdentity(
    tenantId: TenantId,
    userId: UserId,
    roles: Set[String]
)
```

Use `RoleBasedPolicy` to create an `AuthPolicy[CallerIdentity]` with role mappings:

```scala
given AuthPolicy[CallerIdentity] = RoleBasedPolicy {
  case CrudAction.Create => Set("write")
  case CrudAction.Read   => Set("read")
  case CrudAction.Update => Set("write")
  case CrudAction.Delete => Set("admin")
}
```

### SaaSCommand

Wraps your business command with the auth context. This is used as the `CommandMessage` payload type:

```scala
final case class SaaSCommand[Auth, +C](
    auth: Auth,
    payload: C
)
```

### CrudState

Wraps your entity state with tenant and owner information:

```scala
enum CrudState[+A]:
  case NonExistent                                          // before creation
  case Active(tenantId: TenantId, ownerId: UserId, data: A)
  case Deleted(tenantId: TenantId, ownerId: UserId)
```

### CrudAction

The four CRUD operations, used for authorization mapping:

```scala
enum CrudAction:
  case Create, Read, Update, Delete
```

## Defining a CQRS Service

Extend `SaaSCQRSService` instead of `CQRSModel#Service`. You provide your auth type, an `AuthPolicy` given, and a rejection factory:

```scala
import edomata.saas.*
import cats.Monad

// Your business types
case class Todo(title: String, completed: Boolean)
enum TodoCommand:
  case Create(title: String)
  case Complete
  case Delete

// Your CQRS model (state = CrudState[Todo])
object TodoModel extends edomata.core.CQRSModel[CrudState[Todo], String]:
  def initial = CrudState.NonExistent
import TodoModel.given

// Auth policy
given AuthPolicy[CallerIdentity] = RoleBasedPolicy {
  case CrudAction.Create => Set("todo:write")
  case CrudAction.Read   => Set("todo:read")
  case CrudAction.Update => Set("todo:write")
  case CrudAction.Delete => Set("todo:admin")
}

// Your service -- guards are automatic
object TodoService extends SaaSCQRSService[
  CallerIdentity, TodoCommand, Todo, String, String
](mkRejection = identity):
  import SaaS.*

  def apply[F[_]: Monad](): App[F, Unit] = guardedRouter {
    case TodoCommand.Create(title) =>
      (CrudAction.Create, for
        a <- auth
        _ <- set(CrudState.Active(a.tenantId, a.userId, Todo(title, false)))
      yield ())

    case TodoCommand.Complete =>
      (CrudAction.Update, for
        _ <- modifyS {
          case CrudState.Active(tid, oid, todo) =>
            Right(CrudState.Active(tid, oid, todo.copy(completed = true)))
          case other => Left(cats.data.NonEmptyChain.one("Not found"))
        }
      yield ())

    case TodoCommand.Delete =>
      (CrudAction.Delete, for
        state <- entityState
        _ <- state match
          case CrudState.Active(tid, oid, _) =>
            set(CrudState.Deleted(tid, oid))
          case _ => reject("Not found")
      yield ())
  }
```

## Custom Auth Types

You can use any type as your auth context by implementing `AuthPolicy`:

```scala
// Example: JWT claims from your auth middleware
case class JwtClaims(
    sub: String,
    tenantId: String,
    scopes: List[String]
)

given AuthPolicy[JwtClaims] = new AuthPolicy[JwtClaims]:
  def tenantId(auth: JwtClaims): TenantId = TenantId(auth.tenantId)
  def authorize(auth: JwtClaims, action: CrudAction): Either[String, Unit] =
    val requiredScope = action match
      case CrudAction.Create | CrudAction.Update => "write"
      case CrudAction.Read                       => "read"
      case CrudAction.Delete                     => "admin"
    if auth.scopes.contains(requiredScope) then Right(())
    else Left(s"Missing scope: $requiredScope")

// Now use JwtClaims as the auth type in your service
object MyService extends SaaSCQRSService[
  JwtClaims, MyCommand, MyEntity, String, String
](mkRejection = identity):
  // ...
```

Other examples of custom auth types:
- **API key context**: `case class ApiKeyAuth(key: String, tenant: TenantId, isAdmin: Boolean)`
- **OAuth2 token info**: `case class TokenInfo(subject: String, tenant: String, permissions: Set[String])`
- **Internal service auth**: `case class ServiceAuth(serviceName: String, tenant: TenantId)` (always authorized)

## How Guards Work

When you use `guardedRouter`, two checks run **automatically** before each command branch:

1. **Tenant check**: For non-Create actions, verifies that the entity's `tenantId` matches `AuthPolicy.tenantId(auth)`. Returns a rejection on mismatch.

2. **Authorization check**: Calls `AuthPolicy.authorize(auth, action)`. Returns a rejection if denied.

These checks use Edomata's monadic sequencing (`guard >> logic`). If the guard produces a `Rejected` result, the business logic is never executed -- `flatMap` short-circuits automatically.

## Bypassing Guards (Super-Admin)

For administrative endpoints (dashboards, super-admin tools), you can bypass guards using the `unsafe` variants:

```scala
// Write side: bypass tenant + authorization checks
def adminEndpoint[F[_]: Monad](): App[F, Unit] =
  SaaS.unsafeUnguardedRouter {
    case TodoCommand.Delete =>
      // No tenant/auth check -- admin can delete anything
      SaaS.set(CrudState.NonExistent)
    case cmd =>
      // For other commands, re-enable guards manually if needed
      SaaS.guarded(CrudAction.Update)(handleCommand(cmd))
  }
```

The `unsafe` prefix makes every bypass **grep-able** and visible in code reviews. Available bypass methods:

| Method | Description |
|--------|-------------|
| `unsafeUnguardedRouter` | Router with no automatic guards |
| `unsafeUnguarded` | Single-action with no guard |

## Read-Side Queries

Edomata does not handle reads directly. The SaaS module provides type-safe abstractions that make tenant filtering structurally required:

### TenantScopedQuery (default)

The auth context is a required parameter -- you cannot forget the tenant filter:

```scala
import edomata.saas.*

val listTodos: TenantScopedQuery[IO, CallerIdentity, Todo, Unit] =
  TenantScopedQuery[IO, CallerIdentity, Todo, Unit] { (tenantId, _) =>
    // Your SQL always includes WHERE tenant_id = ?
    sql"SELECT data FROM todos WHERE tenant_id = $tenantId"
      .query[Todo].to[List].transact(xa)
  }

// Usage: tenant filtering is automatic via AuthPolicy.tenantId
listTodos.query(callerIdentity, ())
```

### UnsafeCrossTenantQuery (admin bypass)

For admin dashboards that need to query across all tenants:

```scala
val adminListAll: UnsafeCrossTenantQuery[IO, Todo, Unit] =
  UnsafeCrossTenantQuery[IO, Todo, Unit] { _ =>
    sql"SELECT data FROM todos".query[Todo].to[List].transact(xa)
  }

// No auth context needed -- queries all tenants
adminListAll.query(())
```

## Backend Wiring

The SaaS service traits expose a `domain` field compatible with `Backend.builder`:

```scala
import edomata.saas.*

val backend = Backend
  .builder(TodoService.domain)
  .use(driver)
  .build
```

## Build Configuration

For maximum enforcement, your external project should depend **only** on `edomata-saas`:

```scala
// build.sbt
libraryDependencies += "dev.hnaderi" %% "edomata-saas" % "@VERSION@"
// Do NOT add edomata-core -- use edomata.saas.* imports only
```

By importing `edomata.saas.*` instead of `edomata.core.*`, developers get the guarded DSL and cannot accidentally construct unguarded services. The raw `DomainDSL` and `CQRSDomainDSL` are not re-exported.
