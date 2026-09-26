CREATE SCHEMA IF NOT EXISTS "catalog";
CREATE TABLE IF NOT EXISTS "catalog".states (
  id text NOT NULL,
  "version" int8 NOT NULL,
  state bytea NOT NULL,
  tenant_id text NOT NULL,
  owner_id text NOT NULL,
  CONSTRAINT states_pk PRIMARY KEY (id)
);
CREATE INDEX IF NOT EXISTS states_tenant_idx ON "catalog".states (tenant_id);
CREATE INDEX IF NOT EXISTS states_tenant_owner_idx ON "catalog".states (tenant_id, owner_id);
CREATE TABLE IF NOT EXISTS "catalog".outbox (
  seqnr bigserial NOT NULL,
  stream text NOT NULL,
  correlation text NULL,
  causation text NULL,
  payload bytea NOT NULL,
  created timestamptz NOT NULL,
  published timestamptz NULL,
  tenant_id text NOT NULL,
  CONSTRAINT outbox_pk PRIMARY KEY (seqnr)
);
CREATE INDEX IF NOT EXISTS outbox_tenant_idx ON "catalog".outbox (tenant_id);
CREATE TABLE IF NOT EXISTS "catalog".commands (
  id text NOT NULL,
  "time" timestamptz NOT NULL,
  address text NOT NULL,
  CONSTRAINT commands_pk PRIMARY KEY (id)
);
