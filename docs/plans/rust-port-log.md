# Rust port log

What happened when Edomata was ported to Rust, following [`rust-port.md`](rust-port.md), and the
final state of the repository.

## Outcome

The port is complete and merged into `main`. The 10 milestones went in as 10 squash commits,
from #43 to #52. After each merge, `main`'s tree was checked to be byte-identical to the tree CI
had tested for that milestone.

| PR | Squash commit | Milestone |
|----|---------------|-----------|
| #43 | `0b75d07` | 1. Cargo workspace and `edomata-core` |
| #44 | `8430648` | 2. `edomata-backend` and shared backend test suites |
| #45 | `88afb05` | 3. `edomata-postgres`, golden DDL tests |
| #46 | `809da8e` | 4. `edomata-serde`, `jsonb` wire format |
| #47 | `a169fb7` | 5. `edomata-sqlx` driver (replaces Skunk and Doobie) |
| #48 | `0e86e99` | 6. `edomata-testkit`, `edomata-saas`, `edomata-saas-sqlx` |
| #49 | `c475543` | 7. `edomata-simple` (the `java-api` equivalent) |
| #50 | `9bf9b2b` | 8. e2e tests, examples, Scala/Rust cross-language test |
| #51 | `759244b` | 9. Broker distribution: `edomata-broker`, `edomata-kafka`, `edomata-rabbitmq` |
| #52 | `4ebe8cf` | 10. mdBook, migration guide, crate READMEs, CI |

The plan itself went in as #42 (`9975025`).

## Verification on `main` (`4ebe8cf`)

Local run with the docker-compose PostgreSQL 14, Docker, a JDK and sbt:

| Check | Result |
|-------|--------|
| `cargo fmt --all --check` | pass |
| `cargo clippy --workspace --all-targets --all-features -- -D warnings` | pass |
| `cargo test --workspace --all-features` | 97 test binaries, 591 tests, 0 failures |
| Cross-language test (Scala writes, Rust reads and appends, Scala verifies) | 2 passed |
| Golden DDL and golden payload tests | 5 and 6 passed |
| Kafka / RabbitMQ relay integration tests (testcontainers, real brokers) | 5 and 5 passed |
| `cargo build -p edomata-core --target wasm32-unknown-unknown` | pass |
| `cargo tree`: JVM, Scala or Java artifacts | none |
| `mdbook build rust/book` | pass |

CI on `main` after the merges: see [CI on main](#ci-on-main) below.

## Decisions taken at the end

- **Accepted as not ported**, each with the justification in `rust/PORTING.md`:
  - the deprecated Scala `Response` alias;
  - the Cats `Traverse` law set for `ResponseT`, because Rust has no `Traverse` (ADR 0004).
- **Accepted limitation:** Rust can't read payloads written by uPickle's MessagePack codec,
  because `serde_json` is the only serializer. Every other Scala codec output is readable. This is
  documented in the migration guide.
- **Cargo features:** the `kafka` / `rabbitmq` features live on the examples package, not on
  `edomata-broker`, because Cargo can't express them there without a dependency cycle. A CI job
  checks that `edomata-broker` pulls in no broker client.

## How the port ran

1. **Spec and goal** (#42): `rust-port.md`, plus `rust-port.goal.txt`, a condition under the
   4,000-character `/goal` limit.
2. **Run 1**: Claude Code on Fable, with `/goal` in headless `auto` mode. It opened milestones
   1–9 and committed milestone 10. It stopped when the API credit ran out after about 4 hours
   and US$192.91, billed to an `ANTHROPIC_API_KEY` found in the environment.
3. **Run 2**: the same session resumed on Fable under the claude.ai Team plan, without the API
   key. It stopped immediately at the plan's monthly spend limit, because Fable uses usage
   credits. No credits were bought.
4. **Run 3**: the same session resumed on Opus 5.5, which is included in the plan, again without
   the API key (`apiKeySource: none`). It:
   - fixed the milestone 10 audit findings and opened #52;
   - fixed an intermittent test in #51: the leader failover test asserted an empty outbox before
     the relay had committed the marking. It now waits up to 10 s, keeps the same assertion,
     and passed 25 runs in a row;
   - confirmed every goal condition.
5. **Merge and cleanup**: the stacked PRs were squash-merged bottom-up. Before each merge, the
   PR's branch was rebased onto `main` with `git rebase --onto`, starting from the parent
   branch's *original* tip, and pushed only after an empty `git diff` against the tested tip.
   Then:
   - all worktrees, and the local and remote `rust/*` branches, were deleted;
   - the port's run logs were removed.

Every milestone ran the mandatory pre-PR documentation audit, and its findings were fixed in a
dedicated `docs:` commit each time.

### CI noise that isn't caused by the port

These two checks already fail on `main` before the port:
- **`Validate Steward Config`**: a problem with the Scala Steward config.
- **`claude-review`**: the GitHub Action ends after 1 turn at $0, which points to its Anthropic
  secret.

A few Scala jobs failed because of temporary network problems: the Nix cache and the sbt
launcher download. They passed when re-run.

## CI on main

CI on `4ebe8cf`, the last squash commit:

| Workflow | Result |
|----------|--------|
| Rust: fmt, clippy, doc, tests with PostgreSQL, JDK, sbt and Docker; MSRV 1.88; wasm32; mdBook; dependency guards | success |
| Continuous Integration (Scala): tests on JVM (temurin 8 and 17), JS and Native; Generate Site; Submit Dependencies | success |
| Continuous Integration: `Validate Steward Config` | failure (already failing on `main` before the port) |

### Releases triggered by the merges

`Auto Tag on Merge` tags a patch version on every merge to `main`, and each tag starts the
Maven Central publish workflow. The 10 squash merges created the tags `v0.12.25` to
`v0.12.34`:

- **`0.12.25` is on Maven Central**, built from `0b75d07` (milestone 1). The cancel arrived
  while the publish step was already running. Maven Central releases can't be deleted.
- **`0.12.26` to `0.12.34` were not published.** Their publish runs were cancelled, but the tags
  exist on GitHub.
- **`0.12.24`** had already been published when the plan (#42) was merged.

The port adds Rust code, documentation and Scala test tooling only. The Scala library
artifacts in `0.12.24` and `0.12.25` are therefore functionally the same as `0.12.23`.

To stop this from happening again, PR #58 changed `Auto Tag on Merge` so that it tags a release
only when a published artifact changes: a `v0.12.N` tag for a published Scala module, and a
`rust-v0.1.N` tag for a published Rust crate. Documentation, tests, examples, the book, the
website and CI-only changes create no tag (see the "Releases" section of `CLAUDE.md`). The
merges after it, including this log, therefore created no Scala release.

## Left to the maintainers

- **`ANTHROPIC_API_KEY`**: it is exported in the terminal session (Ghostty, then herdr, then
  zsh), but not from the zsh startup files, `launchctl`, or the Ghostty or herdr configs. Unset
  it where it is exported, so local Claude Code runs use the claude.ai plan. To check:
  `claude auth status` should report `"authMethod": "claude.ai"`.
- **Usage credits:** make sure extra usage and usage credits are off at
  claude.ai/admin-settings/usage.
- **Local PostgreSQL:** Homebrew `postgresql@16` was stopped so that `localhost:5432` reaches
  the docker-compose PostgreSQL the tests use. Restart it with
  `brew services start postgresql@16` if you need it, and point `DATABASE_URL` at the
  container when running the Rust tests.
- **Optional follow-ups:**
  - fix `Validate Steward Config` and the `claude-review` secret;
  - decide whether to publish the crates on crates.io;
  - decide whether to propose the port upstream (`hnaderi/edomata`). It only exists on this fork.
