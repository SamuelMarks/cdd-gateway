CREATE TABLE releases (
    id SERIAL PRIMARY KEY,
    repository_id INTEGER NOT NULL REFERENCES repositories(id),
    github_id BIGINT UNIQUE,
    tag_name VARCHAR NOT NULL,
    name VARCHAR,
    body TEXT
);
