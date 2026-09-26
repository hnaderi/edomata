CREATE SCHEMA IF NOT EXISTS "Order_v2$";
CREATE TABLE IF NOT EXISTS "Order_v2$".journal (
  id uuid NOT NULL,
  "time" timestamptz NOT NULL,
  seqnr bigserial NOT NULL,
  "version" int8 NOT NULL,
  stream text NOT NULL,
  payload json NOT NULL,
  CONSTRAINT journal_pk PRIMARY KEY (id),
  CONSTRAINT journal_un UNIQUE (stream, version)
);
CREATE INDEX IF NOT EXISTS journal_seqnr_idx ON "Order_v2$".journal USING btree (seqnr);
CREATE INDEX IF NOT EXISTS journal_stream_idx ON "Order_v2$".journal USING btree (stream, version);
CREATE TABLE IF NOT EXISTS "Order_v2$".outbox (
  seqnr bigserial NOT NULL,
  stream text NOT NULL,
  correlation text NULL,
  causation text NULL,
  payload json NOT NULL,
  created timestamptz NOT NULL,
  published timestamptz NULL,
  CONSTRAINT outbox_pk PRIMARY KEY (seqnr)
);
CREATE TABLE IF NOT EXISTS "Order_v2$".commands (
  id text NOT NULL,
  "time" timestamptz NOT NULL,
  address text NOT NULL,
  CONSTRAINT commands_pk PRIMARY KEY (id)
);
CREATE TABLE IF NOT EXISTS "Order_v2$".snapshots (
  id text NOT NULL,
  "version" int8 NOT NULL,
  state json NOT NULL,
  CONSTRAINT snapshots_pk PRIMARY KEY (id)
);
CREATE TABLE IF NOT EXISTS "Order_v2$".migrations (
  "version" text NOT NULL,
  description text NOT NULL,
  applied_at timestamptz NOT NULL DEFAULT now(),
  CONSTRAINT migrations_pk PRIMARY KEY ("version")
);
