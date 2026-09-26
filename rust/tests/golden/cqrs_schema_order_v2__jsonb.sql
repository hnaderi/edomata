CREATE SCHEMA IF NOT EXISTS "Order_v2$";
CREATE TABLE IF NOT EXISTS "Order_v2$".states (
  id text NOT NULL,
  "version" int8 NOT NULL,
  state jsonb NOT NULL,
  CONSTRAINT states_pk PRIMARY KEY (id)
);
CREATE TABLE IF NOT EXISTS "Order_v2$".outbox (
  seqnr bigserial NOT NULL,
  stream text NOT NULL,
  correlation text NULL,
  causation text NULL,
  payload jsonb NOT NULL,
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
