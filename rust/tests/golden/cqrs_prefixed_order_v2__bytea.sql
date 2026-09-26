CREATE TABLE IF NOT EXISTS order_v2$_states (
  id text NOT NULL,
  "version" int8 NOT NULL,
  state bytea NOT NULL,
  CONSTRAINT order_v2$_states_pk PRIMARY KEY (id)
);
CREATE TABLE IF NOT EXISTS order_v2$_outbox (
  seqnr bigserial NOT NULL,
  stream text NOT NULL,
  correlation text NULL,
  causation text NULL,
  payload bytea NOT NULL,
  created timestamptz NOT NULL,
  published timestamptz NULL,
  CONSTRAINT order_v2$_outbox_pk PRIMARY KEY (seqnr)
);
CREATE TABLE IF NOT EXISTS order_v2$_commands (
  id text NOT NULL,
  "time" timestamptz NOT NULL,
  address text NOT NULL,
  CONSTRAINT order_v2$_commands_pk PRIMARY KEY (id)
);
