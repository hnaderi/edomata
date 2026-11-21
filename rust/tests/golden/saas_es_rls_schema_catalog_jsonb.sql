CREATE SCHEMA IF NOT EXISTS "catalog";
CREATE TABLE IF NOT EXISTS "catalog".journal (
  id uuid NOT NULL,
  "time" timestamptz NOT NULL,
  seqnr bigserial NOT NULL,
  "version" int8 NOT NULL,
  stream text NOT NULL,
  payload jsonb NOT NULL,
  tenant_id text GENERATED ALWAYS AS (CASE WHEN position('/' in stream) > 1 THEN split_part(stream, '/', 1) END) STORED NOT NULL,
  CONSTRAINT journal_pk PRIMARY KEY (id),
  CONSTRAINT journal_un UNIQUE (stream, version)
);
CREATE INDEX IF NOT EXISTS journal_seqnr_idx ON "catalog".journal USING btree (seqnr);
CREATE INDEX IF NOT EXISTS journal_stream_idx ON "catalog".journal USING btree (stream, version);
CREATE INDEX IF NOT EXISTS journal_tenant_idx ON "catalog".journal USING btree (tenant_id, seqnr);
CREATE TABLE IF NOT EXISTS "catalog".outbox (
  seqnr bigserial NOT NULL,
  stream text NOT NULL,
  correlation text NULL,
  causation text NULL,
  payload jsonb NOT NULL,
  created timestamptz NOT NULL,
  published timestamptz NULL,
  tenant_id text GENERATED ALWAYS AS (CASE WHEN position('/' in stream) > 1 THEN split_part(stream, '/', 1) END) STORED NOT NULL,
  CONSTRAINT outbox_pk PRIMARY KEY (seqnr)
);
CREATE INDEX IF NOT EXISTS outbox_tenant_idx ON "catalog".outbox (tenant_id);
CREATE TABLE IF NOT EXISTS "catalog".commands (
  id text NOT NULL,
  "time" timestamptz NOT NULL,
  address text NOT NULL,
  tenant_id text GENERATED ALWAYS AS (CASE WHEN position('/' in id) > 1 THEN split_part(id, '/', 1) END) STORED NOT NULL,
  CONSTRAINT commands_pk PRIMARY KEY (id),
  CONSTRAINT commands_address_tenant_ck CHECK (position('/' in address) > 1 AND split_part(address, '/', 1) = tenant_id)
);
CREATE INDEX IF NOT EXISTS commands_tenant_idx ON "catalog".commands (tenant_id);
CREATE TABLE IF NOT EXISTS "catalog".snapshots (
  id text NOT NULL,
  "version" int8 NOT NULL,
  state jsonb NOT NULL,
  tenant_id text GENERATED ALWAYS AS (CASE WHEN position('/' in id) > 1 THEN split_part(id, '/', 1) END) STORED NOT NULL,
  CONSTRAINT snapshots_pk PRIMARY KEY (id)
);
CREATE INDEX IF NOT EXISTS snapshots_tenant_idx ON "catalog".snapshots (tenant_id);
GRANT USAGE ON SCHEMA "catalog" TO app_tenant, app_system, app_inspection;
ALTER TABLE "catalog".journal ENABLE ROW LEVEL SECURITY;
ALTER TABLE "catalog".journal FORCE ROW LEVEL SECURITY;
CREATE POLICY journal_tenant_policy ON "catalog".journal FOR ALL TO app_tenant
  USING (tenant_id = NULLIF(current_setting('app.tenant_id', true), ''))
  WITH CHECK (tenant_id = NULLIF(current_setting('app.tenant_id', true), ''));
CREATE POLICY journal_system_policy ON "catalog".journal FOR ALL TO app_system
  USING (true)
  WITH CHECK (true);
CREATE POLICY journal_inspection_policy ON "catalog".journal FOR SELECT TO app_inspection
  USING (true);
GRANT SELECT, INSERT ON "catalog".journal TO app_tenant;
GRANT SELECT, INSERT, UPDATE, DELETE ON "catalog".journal TO app_system;
GRANT SELECT ON "catalog".journal TO app_inspection;
GRANT USAGE ON SEQUENCE "catalog".journal_seqnr_seq TO app_tenant, app_system;
ALTER TABLE "catalog".outbox ENABLE ROW LEVEL SECURITY;
ALTER TABLE "catalog".outbox FORCE ROW LEVEL SECURITY;
CREATE POLICY outbox_tenant_policy ON "catalog".outbox FOR ALL TO app_tenant
  USING (tenant_id = NULLIF(current_setting('app.tenant_id', true), ''))
  WITH CHECK (tenant_id = NULLIF(current_setting('app.tenant_id', true), ''));
CREATE POLICY outbox_system_policy ON "catalog".outbox FOR ALL TO app_system
  USING (true)
  WITH CHECK (true);
CREATE POLICY outbox_inspection_policy ON "catalog".outbox FOR SELECT TO app_inspection
  USING (true);
GRANT SELECT, INSERT ON "catalog".outbox TO app_tenant;
GRANT SELECT, INSERT, UPDATE, DELETE ON "catalog".outbox TO app_system;
GRANT SELECT ON "catalog".outbox TO app_inspection;
GRANT USAGE ON SEQUENCE "catalog".outbox_seqnr_seq TO app_tenant, app_system;
ALTER TABLE "catalog".commands ENABLE ROW LEVEL SECURITY;
ALTER TABLE "catalog".commands FORCE ROW LEVEL SECURITY;
CREATE POLICY commands_tenant_policy ON "catalog".commands FOR ALL TO app_tenant
  USING (tenant_id = NULLIF(current_setting('app.tenant_id', true), ''))
  WITH CHECK (tenant_id = NULLIF(current_setting('app.tenant_id', true), ''));
CREATE POLICY commands_system_policy ON "catalog".commands FOR ALL TO app_system
  USING (true)
  WITH CHECK (true);
CREATE POLICY commands_inspection_policy ON "catalog".commands FOR SELECT TO app_inspection
  USING (true);
GRANT SELECT, INSERT ON "catalog".commands TO app_tenant;
GRANT SELECT, INSERT, UPDATE, DELETE ON "catalog".commands TO app_system;
GRANT SELECT ON "catalog".commands TO app_inspection;
ALTER TABLE "catalog".snapshots ENABLE ROW LEVEL SECURITY;
ALTER TABLE "catalog".snapshots FORCE ROW LEVEL SECURITY;
CREATE POLICY snapshots_tenant_policy ON "catalog".snapshots FOR ALL TO app_tenant
  USING (tenant_id = NULLIF(current_setting('app.tenant_id', true), ''))
  WITH CHECK (tenant_id = NULLIF(current_setting('app.tenant_id', true), ''));
CREATE POLICY snapshots_system_policy ON "catalog".snapshots FOR ALL TO app_system
  USING (true)
  WITH CHECK (true);
CREATE POLICY snapshots_inspection_policy ON "catalog".snapshots FOR SELECT TO app_inspection
  USING (true);
GRANT SELECT, INSERT, UPDATE ON "catalog".snapshots TO app_tenant;
GRANT SELECT, INSERT, UPDATE, DELETE ON "catalog".snapshots TO app_system;
GRANT SELECT ON "catalog".snapshots TO app_inspection;
