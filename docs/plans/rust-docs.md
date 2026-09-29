# Goal: document the Edomata Rust library

> **Status (2026-09-27):** started, then paused to save usage. None of PRs A–D is open yet.
> Uncommitted work in progress exists only in local worktrees, on the branches
> `docs/rust-api-docs` (A), `docs/rust-book` (B), `docs/website-rust-section` (C) and
> `ci/publish-rust-docs` (D). To resume, run one agent per PR with the condition in
> [`rust-docs.goal.txt`](rust-docs.goal.txt), preferably one at a time.

Source of truth for the documentation session. The Rust port of Edomata is complete and merged
on `main` of `beyond-scale-group/edomata`. It lives in `rust/`: a Cargo workspace with 14 crates
under `rust/crates/`, examples in `rust/examples/`, and an mdBook in `rust/book/` with its samples
crate in `rust/book/samples/`. Read `CLAUDE.md` (the "Rust Port" section), `rust/README.md`,
`rust/PORTING.md`, `rust/book/src/SUMMARY.md` and `docs/plans/rust-port.md` first.

The work is complete, accurate and published documentation, in four independent PRs.

## Hard rules

- **Never merge a PR, and never push to `main`.** `Auto Tag on Merge` tags every merge to `main`,
  and each tag publishes a release to Maven Central, which can't be undone. Open PRs only. The
  maintainer merges them.
- Each PR branches from `main` in its own worktree (use the native `EnterWorktree` tool). The four
  PRs must not depend on each other. Where one links to another (for example, the website
  linking to the published book), use the final public URLs below.
- Run the mandatory pre-PR documentation audit from `CLAUDE.md` before opening each PR. Fix its
  findings in a dedicated `docs:` commit.
- Don't change the behaviour of Rust code, and don't touch the Scala modules. Documentation
  comments, lint attributes, doc tests, book samples, CI and website files are in scope.
- Never delete, weaken or `#[ignore]` a test or a doc test to get green. Report failures
  truthfully.
- Everything is in English.

Public URLs, which the website must keep serving at its current root:
- Website: `https://beyond-scale-group.github.io/edomata/`
- Book: `https://beyond-scale-group.github.io/edomata/rust/book/`
- API docs: `https://beyond-scale-group.github.io/edomata/rust/api/`, with crate pages such as
  `.../rust/api/edomata_core/`

## PR A: complete API documentation (rustdoc)

- Document **every public item** in all 14 library crates: modules, types, traits, functions,
  methods, fields, enum variants, associated types, constants and macros. Say what each item
  does, its semantics and invariants, its errors and panics (there should be none in library
  code), and link related items.
- **Crate-level docs** (`//!` in each `lib.rs`): the crate's purpose, where it sits among the
  other crates, a runnable example, and its Cargo feature flags. Show feature-gated items on
  docs.rs with `#![cfg_attr(docsrs, feature(doc_auto_cfg))]` and a
  `[package.metadata.docs.rs]` section.
- **Examples:** add runnable doc examples to the core types at least: `Decision`, `ResponseT` /
  `ResponseD`, `Action`, `Edomaton`, `Stomaton`, `DomainModel`, the DSLs, `PGNaming` /
  `PGSchema`, the codecs, the sqlx drivers' `from(..., skip_setup)`, `OutboxRelay`, and the
  simple API. Use `no_run` for examples that need PostgreSQL or a broker, but they must still
  compile.
- **Lock it in:**
  - add `#![warn(missing_docs)]` to every library crate;
  - add `#![warn(rustdoc::broken_intra_doc_links, rustdoc::private_intra_doc_links)]`;
  - make CI fail on any rustdoc warning. `rust.yml` already runs `cargo doc` with
    `-D warnings`; make sure `missing_docs` is covered there and by clippy's `-D warnings`.
- **Checks:**
  - `RUSTDOCFLAGS="-D warnings" cargo doc --workspace --all-features --no-deps`
  - `cargo test --workspace --all-features --doc`
  - `cargo clippy --workspace --all-targets --all-features -- -D warnings`
  - `cargo test --workspace --all-features`

  All must pass. Docker, the docker-compose PostgreSQL, a JDK and sbt are available. If a
  Homebrew PostgreSQL also listens on `localhost:5432`, set `DATABASE_URL` to the container.

## PR B: review and deepen the book (`rust/book/`)

- **Audit every chapter** in `SUMMARY.md` against the code:
  - every type, function, feature flag, command and path it names must exist and behave as
    described;
  - fix every inaccuracy;
  - remove wording carried over from the Scala docs that doesn't apply to Rust.
- **Add chapters.** All code must come from `rust/book/samples` by anchor, so it is compiled and
  tested with the workspace. Add:
  - **Cookbook:** short recipes for common tasks. Define a domain, validate and reject, publish
    notifications, read state, compose programs, handle optimistic concurrency conflicts and
    retries, keep commands idempotent, use snapshots and caching.
  - **Testing:** unit-testing domains with `edomata-testkit`, and integration tests against
    PostgreSQL.
  - **Operations:**
    - schema management with `PGSchema`, `skip_setup` and a Flyway workflow;
    - event migrations;
    - running the outbox and broker relays in production (leader election with advisory
      locks, at-least-once delivery, deduplication by message ID, metrics and `tracing`);
    - multi-tenant SaaS deployment with RLS.
  - **Troubleshooting:** common errors (`VersionConflict`, `MaxRetryExceeded`, codec errors,
    connection and setup problems) and what to do about them.
  - **API reference:** a chapter that links to the published rustdoc for every crate.
- **Checks:** `mdbook build rust/book` and the full `cargo test` pass. Every relative link and
  anchor resolves. Add `mdbook-linkcheck`, or an equivalent check in CI, if it's cheap.

## PR C: website integration (Docusaurus, `docs/` + `website/`)

- Add a **Rust** section to the website:
  - what the port is;
  - how the crates map to the Scala modules;
  - installation (`Cargo.toml` snippets for the main crates and features);
  - a minimal quickstart;
  - the wire-compatibility guarantees (a shared PostgreSQL between Scala and Rust services,
    `jsonb` payloads, and the uPickle MessagePack limitation);
  - links to the book, the API docs and the migration guide at the public URLs above.
- Give it a sidebar entry. Mention the Rust port in `docs/introduction.md` and
  `docs/about/features.md` without changing their claims about the Scala library.
- `docs/` is processed by mdoc and rendered as MDX. Don't use a bare `<`, `{`, `}` or `@`
  outside code spans or code blocks. Rust code blocks are plain fenced `rust` blocks, not
  `scala mdoc`.
- **Checks:** `sbt docs/mdoc` has no new warnings, and `cd website && npm ci && npm run build`
  succeeds.

## PR D: publish the book and API docs online

- The website is deployed by the `Publish site` step of the `Generate Site` job in
  `.github/workflows/ci.yml`. It uses `peaceiris/actions-gh-pages` with
  `publish_dir: website/build` and **`keep_files: false`**, so each deploy replaces the whole
  `gh-pages` branch. A separate deploy of the Rust docs would be wiped by the next site deploy.
- So build the Rust docs **into the same deploy**. In that job, before `Publish site`:
  1. install Rust and mdbook;
  2. run `mdbook build rust/book` and copy the output to `website/build/rust/book/`;
  3. run `cargo doc --workspace --all-features --no-deps`, copy `rust/target/doc` to
     `website/build/rust/api/`, and add an `index.html` that redirects to `edomata_core/`.

  Keep the existing condition that deploys only on pushes to `main`. `ci.yml` is maintained by
  hand ("Forked from the upstream sbt-github-actions generated workflow"). Check `ci.sbt` and
  keep the two consistent if it generates part of this job.
- Link the book and the API docs from `rust/README.md` and the root `README.md`.
- **Checks:**
  - reproduce the combined build locally (website build, then the Rust docs copied in);
  - check that `website/build/index.html`, `website/build/rust/book/index.html` and
    `website/build/rust/api/edomata_core/index.html` all exist, and that relative links work
    when served under `/edomata/`;
  - the PR's CI must be green.

## Definition of done

- PRs A, B, C and D are open against `main`. None of them is merged.
- Each PR's CI is green, apart from two failures that already happen on `main`:
  `Validate Steward Config` and `claude-review`.
- All the checks listed for each PR pass locally on its branch.
- Each PR description lists what it adds, the checks that were run with their results, and
  anything left undone.
