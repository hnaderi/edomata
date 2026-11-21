CREATE TABLE IF NOT EXISTS catalog_journal (
  id uuid NOT NULL,
  "time" timestamptz NOT NULL,
  seqnr bigserial NOT NULL,
  "version" int8 NOT NULL,
  stream text NOT NULL,
  payload bytea NOT NULL,
  tenant_id text GENERATED ALWAYS AS (CASE WHEN position('/' in stream) > 1 THEN split_part(stream, '/', 1) END) STORED NOT NULL,
  CONSTRAINT catalog_journal_pk PRIMARY KEY (id),
  CONSTRAINT catalog_journal_un UNIQUE (stream, version)
);
CREATE INDEX IF NOT EXISTS catalog_journal_seqnr_idx ON catalog_journal USING btree (seqnr);
CREATE INDEX IF NOT EXISTS catalog_journal_stream_idx ON catalog_journal USING btree (stream, version);
CREATE INDEX IF NOT EXISTS catalog_journal_tenant_idx ON catalog_journal USING btree (tenant_id, seqnr);
CREATE TABLE IF NOT EXISTS catalog_outbox (
  seqnr bigserial NOT NULL,
  stream text NOT NULL,
  correlation text NULL,
  causation text NULL,
  payload json NOT NULL,
  created timestamptz NOT NULL,
  published timestamptz NULL,
  tenant_id text GENERATED ALWAYS AS (CASE WHEN position('/' in stream) > 1 THEN split_part(stream, '/', 1) END) STORED NOT NULL,
  CONSTRAINT catalog_outbox_pk PRIMARY KEY (seqnr)
);
CREATE INDEX IF NOT EXISTS catalog_outbox_tenant_idx ON catalog_outbox (tenant_id);
CREATE TABLE IF NOT EXISTS catalog_commands (
  id text NOT NULL,
  "time" timestamptz NOT NULL,
  address text NOT NULL,
  tenant_id text GENERATED ALWAYS AS (CASE WHEN position('/' in id) > 1 THEN split_part(id, '/', 1) END) STORED NOT NULL,
  CONSTRAINT catalog_commands_pk PRIMARY KEY (id),
  CONSTRAINT catalog_commands_address_tenant_ck CHECK (position('/' in address) > 1 AND split_part(address, '/', 1) = tenant_id)
);
CREATE INDEX IF NOT EXISTS catalog_commands_tenant_idx ON catalog_commands (tenant_id);
CREATE TABLE IF NOT EXISTS catalog_snapshots (
  id text NOT NULL,
  "version" int8 NOT NULL,
  state jsonb NOT NULL,
  tenant_id text GENERATED ALWAYS AS (CASE WHEN position('/' in id) > 1 THEN split_part(id, '/', 1) END) STORED NOT NULL,
  CONSTRAINT catalog_snapshots_pk PRIMARY KEY (id)
);
CREATE INDEX IF NOT EXISTS catalog_snapshots_tenant_idx ON catalog_snapshots (tenant_id);
ALTER TABLE catalog_journal ENABLE ROW LEVEL SECURITY;
ALTER TABLE catalog_journal FORCE ROW LEVEL SECURITY;
CREATE POLICY catalog_journal_tenant_policy ON catalog_journal FOR ALL TO app_tenant
  USING (tenant_id = NULLIF(current_setting('app.tenant_id', true), ''))
  WITH CHECK (tenant_id = NULLIF(current_setting('app.tenant_id', true), ''));
CREATE POLICY catalog_journal_system_policy ON catalog_journal FOR ALL TO app_system
  USING (true)
  WITH CHECK (true);
CREATE POLICY catalog_journal_inspection_policy ON catalog_journal FOR SELECT TO app_inspection
  USING (true);
GRANT SELECT, INSERT ON catalog_journal TO app_tenant;
GRANT SELECT, INSERT, UPDATE, DELETE ON catalog_journal TO app_system;
GRANT SELECT ON catalog_journal TO app_inspection;
GRANT USAGE ON SEQUENCE catalog_journal_seqnr_seq TO app_tenant, app_system;
ALTER TABLE catalog_outbox ENABLE ROW LEVEL SECURITY;
ALTER TABLE catalog_outbox FORCE ROW LEVEL SECURITY;
CREATE POLICY catalog_outbox_tenant_policy ON catalog_outbox FOR ALL TO app_tenant
  USING (tenant_id = NULLIF(current_setting('app.tenant_id', true), ''))
  WITH CHECK (tenant_id = NULLIF(current_setting('app.tenant_id', true), ''));
CREATE POLICY catalog_outbox_system_policy ON catalog_outbox FOR ALL TO app_system
  USING (true)
  WITH CHECK (true);
CREATE POLICY catalog_outbox_inspection_policy ON catalog_outbox FOR SELECT TO app_inspection
  USING (true);
GRANT SELECT, INSERT ON catalog_outbox TO app_tenant;
GRANT SELECT, INSERT, UPDATE, DELETE ON catalog_outbox TO app_system;
GRANT SELECT ON catalog_outbox TO app_inspection;
GRANT USAGE ON SEQUENCE catalog_outbox_seqnr_seq TO app_tenant, app_system;
ALTER TABLE catalog_commands ENABLE ROW LEVEL SECURITY;
ALTER TABLE catalog_commands FORCE ROW LEVEL SECURITY;
CREATE POLICY catalog_commands_tenant_policy ON catalog_commands FOR ALL TO app_tenant
  USING (tenant_id = NULLIF(current_setting('app.tenant_id', true), ''))
  WITH CHECK (tenant_id = NULLIF(current_setting('app.tenant_id', true), ''));
CREATE POLICY catalog_commands_system_policy ON catalog_commands FOR ALL TO app_system
  USING (true)
  WITH CHECK (true);
CREATE POLICY catalog_commands_inspection_policy ON catalog_commands FOR SELECT TO app_inspection
  USING (true);
GRANT SELECT, INSERT ON catalog_commands TO app_tenant;
GRANT SELECT, INSERT, UPDATE, DELETE ON catalog_commands TO app_system;
GRANT SELECT ON catalog_commands TO app_inspection;
ALTER TABLE catalog_snapshots ENABLE ROW LEVEL SECURITY;
ALTER TABLE catalog_snapshots FORCE ROW LEVEL SECURITY;
CREATE POLICY catalog_snapshots_tenant_policy ON catalog_snapshots FOR ALL TO app_tenant
  USING (tenant_id = NULLIF(current_setting('app.tenant_id', true), ''))
  WITH CHECK (tenant_id = NULLIF(current_setting('app.tenant_id', true), ''));
CREATE POLICY catalog_snapshots_system_policy ON catalog_snapshots FOR ALL TO app_system
  USING (true)
  WITH CHECK (true);
CREATE POLICY catalog_snapshots_inspection_policy ON catalog_snapshots FOR SELECT TO app_inspection
  USING (true);
GRANT SELECT, INSERT, UPDATE ON catalog_snapshots TO app_tenant;
GRANT SELECT, INSERT, UPDATE, DELETE ON catalog_snapshots TO app_system;
GRANT SELECT ON catalog_snapshots TO app_inspection;
