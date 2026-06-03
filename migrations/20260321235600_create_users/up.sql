CREATE TABLE users (
    id SERIAL PRIMARY KEY,
    github_id BIGINT UNIQUE,
    username VARCHAR NOT NULL UNIQUE,
    email VARCHAR NOT NULL UNIQUE,
    password_hash VARCHAR
);
