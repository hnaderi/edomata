CREATE SCHEMA IF NOT EXISTS "Shop_v2$";
CREATE TABLE IF NOT EXISTS "Shop_v2$".states (
  id text NOT NULL,
  "version" int8 NOT NULL,
  state jsonb NOT NULL,
  tenant_id text NOT NULL,
  owner_id text NOT NULL,
  CONSTRAINT states_pk PRIMARY KEY (id)
);
CREATE INDEX IF NOT EXISTS states_tenant_idx ON "Shop_v2$".states (tenant_id);
CREATE INDEX IF NOT EXISTS states_tenant_owner_idx ON "Shop_v2$".states (tenant_id, owner_id);
CREATE TABLE IF NOT EXISTS "Shop_v2$".outbox (
  seqnr bigserial NOT NULL,
  stream text NOT NULL,
  correlation text NULL,
  causation text NULL,
  payload jsonb NOT NULL,
  created timestamptz NOT NULL,
  published timestamptz NULL,
  tenant_id text NOT NULL,
  CONSTRAINT outbox_pk PRIMARY KEY (seqnr)
);
CREATE INDEX IF NOT EXISTS outbox_tenant_idx ON "Shop_v2$".outbox (tenant_id);
CREATE TABLE IF NOT EXISTS "Shop_v2$".commands (
  id text NOT NULL,
  "time" timestamptz NOT NULL,
  address text NOT NULL,
  CONSTRAINT commands_pk PRIMARY KEY (id)
);
ALTER TABLE "Shop_v2$".states ENABLE ROW LEVEL SECURITY;
CREATE POLICY states_tenant_policy ON "Shop_v2$".states
  USING (tenant_id = current_setting('app.tenant_id'));
GRANT SELECT, INSERT, UPDATE ON "Shop_v2$".states TO app_user;
ALTER TABLE "Shop_v2$".outbox ENABLE ROW LEVEL SECURITY;
CREATE POLICY outbox_tenant_policy ON "Shop_v2$".outbox
  USING (tenant_id = current_setting('app.tenant_id'));
GRANT SELECT, INSERT, UPDATE ON "Shop_v2$".outbox TO app_user;
