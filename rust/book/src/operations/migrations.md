# Event migrations in production

The [Event migrations](../tutorials/migrations.md) tutorial explains how to write an `EventMigration` and test it. This chapter is about rolling one out: `SqlxMigrations::run` rewrites journal payloads **in place**, so treat it like any other data migration.

## What a run does

For each migration of the list whose version is not in the `migrations` table, in list order, one transaction:

1. reads every row of the journal (`id`, `payload` as text);
2. applies the transformation to each payload and updates the row (by id, `batch_size` rows at a time, default 500);
3. records the version and description in the `migrations` table;
4. truncates the `snapshots` table, since snapshots hold states folded from the old payloads.

A failing transformation (the function returns `Err`, for instance on a payload it cannot parse) rolls that migration back and stops the run with a `BackendError`; the migrations before it stay applied. Only `json` and `jsonb` payload columns can be migrated.

## Rolling out

1. **Keep old and new code compatible for the duration of the rollout.** A migration changes stored payloads while old instances may still be running. The safest sequence is to deploy readers that accept both formats first (for instance with `#[serde(default)]` on a new field), then run the migration, then remove the old format from the code.
2. **Run it once, before new instances handle commands.** Calling `SqlxMigrations::run` on every start-up is safe (applied versions are skipped), but two instances starting together may both try the same pending migration; the second one fails on the primary key of the `migrations` table after doing the work, and its transaction is rolled back. Run migrations from a single job (a Kubernetes init job, a release step) when several replicas start at once.
3. **The tables must exist.** The runner creates the `migrations` table only; the journal and the `snapshots` table must exist (created by Flyway or a previous start), see [Schema management](schema.md).
4. **Back up first, and try it on a copy of production data.** The old payloads are overwritten.
5. **Mind the size.** A migration rewrites the whole journal in one transaction: on a large journal, plan for the time, the lock on the updated rows and the WAL volume, or migrate during a quiet period.
6. **Expect a cold start.** Snapshots are truncated, so the first command on each aggregate refolds its journal.

## Coordinating with Scala services

A journal shared with Scala services is migrated by one side only. The Scala `SkunkMigrations` / `DoobieMigrations` and the Rust `SqlxMigrations` use the same `migrations` table: a version applied by one is skipped by the other. Give the migrations the same version strings on both sides.
