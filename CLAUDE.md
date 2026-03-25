# CLAUDE.md

This file provides guidance to Claude Code when working with the Edomata codebase.

## Project Overview

Edomata is a lightweight, purely functional Scala 3 library for implementing event-driven automata. It provides abstractions for event sourcing and CQRS patterns, built on the Typelevel ecosystem (Cats, Cats Effect, FS2).

**Author**: Hossein Naderi
**License**: Apache 2.0
**Platforms**: JVM, Scala.js, Scala Native

## Build System

- **Build tool**: SBT 1.12.1
- **Scala version**: 3.3.6
- **Cross-compilation**: JVM, JS, Native platforms

### Common Commands

```bash
# Compile all modules
sbt compile

# Run all tests
sbt test

# Pre-commit checks (format, headers, compile, test)
sbt precommit

# Full release checklist (clean, format check, compile, test)
sbt commit

# Generate documentation
sbt docs/mdoc

# Format code
sbt scalafmtAll

# Start PostgreSQL for integration tests
docker-compose up -d
```

## Project Structure

```
modules/
├── core/              # Core abstractions (Decision, Edomaton, Stomaton)
├── backend/           # Event sourcing backend abstractions
├── postgres/          # PostgreSQL common components
├── skunk/             # Skunk-based PostgreSQL backend (cross-platform)
├── skunk-circe/       # Circe JSON codecs for Skunk
├── skunk-jsoniter/    # Jsoniter codecs for Skunk
├── skunk-upickle/     # uPickle codecs for Skunk
├── doobie/            # Doobie-based PostgreSQL backend (JVM only)
├── doobie-circe/      # Circe JSON codecs for Doobie
├── doobie-jsoniter/   # Jsoniter codecs for Doobie
├── doobie-upickle/    # uPickle codecs for Doobie
├── backend-tests/     # Integration tests
├── e2e/               # End-to-end tests
├── munit/             # MUnit test framework integration
└── examples/          # Example implementations

docs/                  # Documentation (Markdown)
site/                  # Documentation site generator
```

## Core Abstractions

| Type | Purpose |
|------|---------|
| `Decision[R, E, A]` | State machine with Accept/Reject/Indecisive outcomes |
| `Response[E, A]` | Decision combined with event publishing |
| `Edomaton[M, C, E, R, S]` | Event-driven automaton (full event sourcing) |
| `Stomaton[M, C, R, S]` | State-only automaton (CQRS without event sourcing) |
| `DecisionT[F, R, E, A]` | Effectful decision transformer |
| `Action[F, E, R, A]` | Effect runner yielding responses |

## Key Dependencies

- **Cats** 2.10.0 - Functional programming abstractions
- **Cats Effect** 3.6.3 - IO and effect handling
- **FS2** 3.12.2 - Functional streams
- **Skunk** 0.6.5 - Async PostgreSQL client
- **Doobie** 1.0.0-RC11 - JDBC-based database layer
- **Circe** 0.14.15 - JSON serialization
- **MUnit** 1.0.0-M8 - Testing framework

## Testing

Tests require a PostgreSQL database. Use Docker Compose to start one:

```bash
docker-compose up -d
```

This starts PostgreSQL 14 on port 5432 with:
- Database: `test`
- User: `test`
- Password: `test`

Run tests with:

```bash
sbt test                    # All tests
sbt coreJVM/test           # Core module only (JVM)
sbt skunkJVM/test          # Skunk backend tests
sbt doobieJVM/test         # Doobie backend tests
sbt e2eJVM/test            # End-to-end tests
```

## Code Style

- **Formatter**: Scalafmt 3.10.4
- **Config**: `.scalafmt.conf`
- **Dialect**: Scala 3

Run formatter before committing:

```bash
sbt scalafmtAll
```

## Development Environment

The project uses Nix/Direnv for environment management. If you have Nix installed:

```bash
direnv allow
```

This provides JDK and SBT automatically.

## Module Dependencies

```
core
 └── backend
      └── postgres
           ├── skunk (cross-platform)
           │    ├── skunk-circe
           │    ├── skunk-jsoniter
           │    └── skunk-upickle
           └── doobie (JVM only)
                ├── doobie-circe
                ├── doobie-jsoniter
                └── doobie-upickle
```

## CI/CD

GitHub Actions runs on:
- JVM versions: temurin@8, temurin@17
- Platforms: JVM, JS, Native
- Checks: format, compile, test
