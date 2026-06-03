CREATE TABLE repositories (
    id SERIAL PRIMARY KEY,
    organization_id INTEGER NOT NULL REFERENCES organizations(id),
    github_id BIGINT UNIQUE,
    name VARCHAR NOT NULL,
    description TEXT
);
