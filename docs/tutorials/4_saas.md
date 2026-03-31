# SaaS Multi-Tenant Module

## Overview

The `edomata-saas` module provides generic multi-tenant CRUD abstractions that automatically enforce **tenant isolation** and **role-based authorization** on both reads and writes. When you extend the SaaS service traits, every command is guarded before business logic runs -- no manual checks needed.

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

### CallerIdentity

Represents the authenticated caller, typically extracted from a JWT or API key at the HTTP boundary:

```scala
final case class CallerIdentity(
    tenantId: TenantId,
    userId: UserId,
    roles: Set[String]
)
```

### SaaSCommand

Wraps your business command with the caller identity. This is used as the `CommandMessage` payload type:

```scala
final case class SaaSCommand[+C](
    caller: CallerIdentity,
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

The four CRUD operations, used for role mapping:

```scala
enum CrudAction:
  case Create, Read, Update, Delete
```

## Defining a CQRS Service

Extend `SaaSCQRSService` instead of `CQRSModel#Service`. You provide a role mapping and a rejection factory:

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

// Your service -- guards are automatic
object TodoService extends SaaSCQRSService[TodoCommand, Todo, String, String](
  rolesFor = {
    case CrudAction.Create => Set("todo:write")
    case CrudAction.Read   => Set("todo:read")
    case CrudAction.Update => Set("todo:write")
    case CrudAction.Delete => Set("todo:admin")
  },
  mkRejection = identity  // String -> String, no mapping needed
):
  import SaaS.*

  def apply[F[_]: Monad](): App[F, Unit] = guardedRouter {
    case TodoCommand.Create(title) =>
      (CrudAction.Create, for
        c <- caller
        _ <- set(CrudState.Active(c.tenantId, c.userId, Todo(title, false)))
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

## How Guards Work

When you use `guardedRouter`, two checks run **automatically** before each command branch:

1. **Tenant check**: For non-Create actions, verifies that the entity's `tenantId` matches the caller's `tenantId`. Returns a rejection on mismatch.

2. **Role check**: Verifies that the caller's `roles` contain all roles required for the action (as defined by your `rolesFor` mapping). Returns a rejection if roles are missing.

These checks use Edomata's monadic sequencing (`guard >> logic`). If the guard produces a `Rejected` result, the business logic is never executed -- `flatMap` short-circuits automatically.

## Bypassing Guards (Super-Admin)

For administrative endpoints (dashboards, super-admin tools), you can bypass guards using the `unsafe` variants:

```scala
// Write side: bypass tenant + role checks
def adminEndpoint[F[_]: Monad](): App[F, Unit] =
  SaaS.unsafeUnguardedRouter {
    case TodoCommand.Delete =>
      // No tenant/role check -- admin can delete anything
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

`CallerIdentity` is a required parameter -- you cannot forget the tenant filter:

```scala
import edomata.saas.*

val listTodos: TenantScopedQuery[IO, Todo, Unit] =
  TenantScopedQuery[IO, Todo, Unit] { (tenantId, _) =>
    // Your SQL always includes WHERE tenant_id = ?
    sql"SELECT data FROM todos WHERE tenant_id = $tenantId"
      .query[Todo].to[List].transact(xa)
  }

// Usage: tenant filtering is automatic
listTodos.query(callerIdentity, ())
```

### UnsafeCrossTenantQuery (admin bypass)

For admin dashboards that need to query across all tenants:

```scala
val adminListAll: UnsafeCrossTenantQuery[IO, Todo, Unit] =
  UnsafeCrossTenantQuery[IO, Todo, Unit] { _ =>
    sql"SELECT data FROM todos".query[Todo].to[List].transact(xa)
  }

// No caller identity needed -- queries all tenants
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
