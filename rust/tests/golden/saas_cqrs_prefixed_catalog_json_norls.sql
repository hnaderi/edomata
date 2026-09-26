CREATE TABLE IF NOT EXISTS catalog_states (
  id text NOT NULL,
  "version" int8 NOT NULL,
  state json NOT NULL,
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
  payload json NOT NULL,
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
