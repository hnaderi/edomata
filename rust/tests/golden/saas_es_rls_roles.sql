DO $$ BEGIN IF NOT EXISTS (SELECT FROM pg_roles WHERE rolname = 'app_tenant') THEN CREATE ROLE app_tenant NOLOGIN NOSUPERUSER NOBYPASSRLS; END IF; END $$;
DO $$ BEGIN IF NOT EXISTS (SELECT FROM pg_roles WHERE rolname = 'app_system') THEN CREATE ROLE app_system NOLOGIN NOSUPERUSER NOBYPASSRLS; END IF; END $$;
DO $$ BEGIN IF NOT EXISTS (SELECT FROM pg_roles WHERE rolname = 'app_inspection') THEN CREATE ROLE app_inspection NOLOGIN NOSUPERUSER NOBYPASSRLS; END IF; END $$;
GRANT app_tenant TO app_web;
GRANT app_system TO app_relay;
GRANT app_inspection TO app_agent;
