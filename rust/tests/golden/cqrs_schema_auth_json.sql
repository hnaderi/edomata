CREATE SCHEMA IF NOT EXISTS "auth";
CREATE TABLE IF NOT EXISTS "auth".states (
  id text NOT NULL,
  "version" int8 NOT NULL,
  state json NOT NULL,
  CONSTRAINT states_pk PRIMARY KEY (id)
);
CREATE TABLE IF NOT EXISTS "auth".outbox (
  seqnr bigserial NOT NULL,
  stream text NOT NULL,
  correlation text NULL,
  causation text NULL,
  payload json NOT NULL,
  created timestamptz NOT NULL,
  published timestamptz NULL,
  CONSTRAINT outbox_pk PRIMARY KEY (seqnr)
);
CREATE TABLE IF NOT EXISTS "auth".commands (
  id text NOT NULL,
  "time" timestamptz NOT NULL,
  address text NOT NULL,
  CONSTRAINT commands_pk PRIMARY KEY (id)
);
