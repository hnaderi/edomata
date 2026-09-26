CREATE TABLE IF NOT EXISTS order_v2$_journal (
  id uuid NOT NULL,
  "time" timestamptz NOT NULL,
  seqnr bigserial NOT NULL,
  "version" int8 NOT NULL,
  stream text NOT NULL,
  payload jsonb NOT NULL,
  CONSTRAINT order_v2$_journal_pk PRIMARY KEY (id),
  CONSTRAINT order_v2$_journal_un UNIQUE (stream, version)
);
CREATE INDEX IF NOT EXISTS order_v2$_journal_seqnr_idx ON order_v2$_journal USING btree (seqnr);
CREATE INDEX IF NOT EXISTS order_v2$_journal_stream_idx ON order_v2$_journal USING btree (stream, version);
CREATE TABLE IF NOT EXISTS order_v2$_outbox (
  seqnr bigserial NOT NULL,
  stream text NOT NULL,
  correlation text NULL,
  causation text NULL,
  payload jsonb NOT NULL,
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
CREATE TABLE IF NOT EXISTS order_v2$_snapshots (
  id text NOT NULL,
  "version" int8 NOT NULL,
  state jsonb NOT NULL,
  CONSTRAINT order_v2$_snapshots_pk PRIMARY KEY (id)
);
CREATE TABLE IF NOT EXISTS order_v2$_migrations (
  "version" text NOT NULL,
  description text NOT NULL,
  applied_at timestamptz NOT NULL DEFAULT now(),
  CONSTRAINT order_v2$_migrations_pk PRIMARY KEY ("version")
);
