CREATE SCHEMA IF NOT EXISTS "Shop_v2$";
CREATE TABLE IF NOT EXISTS "Shop_v2$".states (
  id text NOT NULL,
  "version" int8 NOT NULL,
  state json NOT NULL,
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
  payload json NOT NULL,
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
