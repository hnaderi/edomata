# ADR 0013: Documentation

- Status: accepted
- Date: 2026-09-25
- Milestone: 10 (documentation)

## Context

The Scala library documents itself with an mdoc-compiled Docusaurus site
(`docs/`, `website/`): tutorials (getting started, event sourcing, CQRS,
backends, processes, SaaS, migrations), principles, backend pages, a Java
API page, a module list and a FAQ. The plan asks for an mdBook under
`rust/book/` covering the same tutorials with compiled samples, a "Simple
API" chapter in place of the Java API page, a "Distributing events with
Kafka / RabbitMQ" chapter, crate READMEs, the porting map, a migration
guide for Scala and Java users, and CI.

## Decisions

1. **mdBook, samples compiled by Cargo.** mdBook has no mdoc: code in the
   chapters is not compiled by the book build. Instead, every sample lives
   in a workspace member, `rust/book/samples` (`edomata-book-samples`, not
   published), one module per chapter, delimited with `// ANCHOR: name`
   markers and pulled into the Markdown with mdBook's include directive
   (`#include ../../samples/src/<module>.rs:name` between double braces).
   `cargo clippy --workspace --all-targets` checks every sample (the
   test-only ones included) and `cargo build --workspace` the non-test
   ones; the pure ones (decisions, models, services,
   guards, migrations, the simple facade, the test kit) are exercised by
   `cargo test -p edomata-book-samples`; the ones that need PostgreSQL or a
   broker are compiled but not run. The broker chapter includes the real
   examples (`rust/examples/src/bin/kafka_relay.rs`, `rabbitmq_relay.rs`)
   through anchors too. The one exception is the `AuthPolicy` trait
   definition quoted in the SaaS chapter. Code blocks are marked `rust,ignore` so that
   `mdbook test` does not try to compile them without dependencies; the
   crate is the compiler.

2. **One book for prose, rustdoc for the API.** The chapters explain
   concepts and show how the pieces fit, and point at the rustdoc of the
   crates for signatures. The book never restates method lists that
   rustdoc already has, except in the mapping tables that Scala users need.

3. **The port's own documents are included, not duplicated.**
   `PORTING.md` and the ADRs are the single source of truth; the book's
   "Porting map" and "Design decisions" chapters include them from
   `rust/PORTING.md` and `rust/docs/adr/*.md` with the same directive.

4. **Structure follows the Scala sidebar.** Introduction, About, Tutorials,
   Principles, Backends, Other, with two replacements required by the
   plan: "Simple API" for the Java API page and "PostgreSQL (sqlx)" for
   the Skunk and Doobie pages, and two additions: "Distributing events
   with Kafka / RabbitMQ" and the "Migration guide for Scala and Java
   users". Diagrams (PlantUML / Mermaid in Scala) are plain-text figures,
   which render everywhere and need no plugin.

5. **Every crate has a README.** Short and uniform: purpose, the Scala
   module it ports, the main types, a pointer to the book chapter and to
   the rustdoc. `rust/README.md` remains the workspace entry point and
   links to the book and the migration guide.

6. **CI builds the book.** A `book` job installs `mdbook` and runs
   `mdbook build book` (from `rust/`); the samples are already covered by the lint
   and test jobs since they are workspace members.

## Consequences

Documentation drift is caught by the compiler (samples), by `mdbook build`
(broken includes) and by the pre-PR documentation audit (prose
against code). The price is that prose and samples live in two files per
chapter, which the anchors keep close.
