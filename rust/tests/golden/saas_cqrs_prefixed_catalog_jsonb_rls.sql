CREATE TABLE IF NOT EXISTS catalog_states (
  id text NOT NULL,
  "version" int8 NOT NULL,
  state jsonb NOT NULL,
  tenant_id text NOT NULL,
  owner_id text NOT NULL,
  CONSTRAINT catalog_states_pk PRIMARY KEY (id)
);
CREATE INDEX IF NOT EXISTS catalog_states_tenant_idx ON catalog_states (tenant_id);
CREATE INDEX IF NOT EXISTS catalog_states_tenant_owner_idx ON catalog_states (tenant_id, owner_id);
CREATE TABLE IF NOT EXISTS catalog_outbox (
  seqnr bigserial NOT NULL,
  stream text NOT NULL,
  correlation text NULL,
  causation text NULL,
  payload jsonb NOT NULL,
  created timestamptz NOT NULL,
  published timestamptz NULL,
  tenant_id text NOT NULL,
  CONSTRAINT catalog_outbox_pk PRIMARY KEY (seqnr)
);
CREATE INDEX IF NOT EXISTS catalog_outbox_tenant_idx ON catalog_outbox (tenant_id);
CREATE TABLE IF NOT EXISTS catalog_commands (
  id text NOT NULL,
  "time" timestamptz NOT NULL,
  address text NOT NULL,
  CONSTRAINT catalog_commands_pk PRIMARY KEY (id)
);
ALTER TABLE catalog_states ENABLE ROW LEVEL SECURITY;
CREATE POLICY catalog_states_tenant_policy ON catalog_states
  USING (tenant_id = current_setting('app.tenant_id'));
GRANT SELECT, INSERT, UPDATE ON catalog_states TO app_user;
ALTER TABLE catalog_outbox ENABLE ROW LEVEL SECURITY;
CREATE POLICY catalog_outbox_tenant_policy ON catalog_outbox
  USING (tenant_id = current_setting('app.tenant_id'));
GRANT SELECT, INSERT, UPDATE ON catalog_outbox TO app_user;
