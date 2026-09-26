CREATE TABLE IF NOT EXISTS myapp_states (
  id text NOT NULL,
  "version" int8 NOT NULL,
  state json NOT NULL,
  CONSTRAINT myapp_states_pk PRIMARY KEY (id)
);
CREATE TABLE IF NOT EXISTS myapp_outbox (
  seqnr bigserial NOT NULL,
  stream text NOT NULL,
  correlation text NULL,
  causation text NULL,
  payload json NOT NULL,
  created timestamptz NOT NULL,
  published timestamptz NULL,
  CONSTRAINT myapp_outbox_pk PRIMARY KEY (seqnr)
);
CREATE TABLE IF NOT EXISTS myapp_commands (
  id text NOT NULL,
  "time" timestamptz NOT NULL,
  address text NOT NULL,
  CONSTRAINT myapp_commands_pk PRIMARY KEY (id)
);
