ALTER TABLE repositories
    DROP COLUMN docs_config_json,
    DROP COLUMN enable_pgp_signing,
    DROP COLUMN release_registries_json,
    DROP COLUMN language_options_json,
    DROP COLUMN target_languages_json,
    DROP COLUMN topology_mode,
    DROP COLUMN github_org_name;

ALTER TABLE users
    DROP COLUMN google_id;
