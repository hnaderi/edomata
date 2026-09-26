CREATE TABLE IF NOT EXISTS myapp_journal (
  id uuid NOT NULL,
  "time" timestamptz NOT NULL,
  seqnr bigserial NOT NULL,
  "version" int8 NOT NULL,
  stream text NOT NULL,
  payload jsonb NOT NULL,
  CONSTRAINT myapp_journal_pk PRIMARY KEY (id),
  CONSTRAINT myapp_journal_un UNIQUE (stream, version)
);
CREATE INDEX IF NOT EXISTS myapp_journal_seqnr_idx ON myapp_journal USING btree (seqnr);
CREATE INDEX IF NOT EXISTS myapp_journal_stream_idx ON myapp_journal USING btree (stream, version);
CREATE TABLE IF NOT EXISTS myapp_outbox (
  seqnr bigserial NOT NULL,
  stream text NOT NULL,
  correlation text NULL,
  causation text NULL,
  payload jsonb NOT NULL,
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
CREATE TABLE IF NOT EXISTS myapp_snapshots (
  id text NOT NULL,
  "version" int8 NOT NULL,
  state jsonb NOT NULL,
  CONSTRAINT myapp_snapshots_pk PRIMARY KEY (id)
);
CREATE TABLE IF NOT EXISTS myapp_migrations (
  "version" text NOT NULL,
  description text NOT NULL,
  applied_at timestamptz NOT NULL DEFAULT now(),
  CONSTRAINT myapp_migrations_pk PRIMARY KEY ("version")
);
