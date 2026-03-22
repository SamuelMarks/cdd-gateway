CREATE TABLE organizations (
    id SERIAL PRIMARY KEY,
    github_id BIGINT UNIQUE,
    login VARCHAR NOT NULL UNIQUE,
    description TEXT
);
