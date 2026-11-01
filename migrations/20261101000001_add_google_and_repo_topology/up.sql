ALTER TABLE users
    ADD COLUMN IF NOT EXISTS google_id VARCHAR UNIQUE;

ALTER TABLE repositories
    ADD COLUMN IF NOT EXISTS github_org_name VARCHAR NOT NULL DEFAULT '',
    ADD COLUMN IF NOT EXISTS topology_mode VARCHAR NOT NULL DEFAULT 'monorepo',
    ADD COLUMN IF NOT EXISTS target_languages_json TEXT NOT NULL DEFAULT '[]',
    ADD COLUMN IF NOT EXISTS language_options_json TEXT NOT NULL DEFAULT '{}',
    ADD COLUMN IF NOT EXISTS release_registries_json TEXT NOT NULL DEFAULT '[]',
    ADD COLUMN IF NOT EXISTS enable_pgp_signing BOOLEAN NOT NULL DEFAULT FALSE,
    ADD COLUMN IF NOT EXISTS docs_config_json TEXT NOT NULL DEFAULT '{}';

ALTER TABLE repositories
    ALTER COLUMN target_languages_json TYPE TEXT USING target_languages_json::TEXT,
    ALTER COLUMN target_languages_json SET DEFAULT '[]',
    ALTER COLUMN language_options_json TYPE TEXT USING language_options_json::TEXT,
    ALTER COLUMN language_options_json SET DEFAULT '{}',
    ALTER COLUMN release_registries_json TYPE TEXT USING release_registries_json::TEXT,
    ALTER COLUMN release_registries_json SET DEFAULT '[]',
    ALTER COLUMN docs_config_json TYPE TEXT USING docs_config_json::TEXT,
    ALTER COLUMN docs_config_json SET DEFAULT '{}';
