CREATE TABLE organization_users (
    organization_id INTEGER NOT NULL REFERENCES organizations(id),
    user_id INTEGER NOT NULL REFERENCES users(id),
    role VARCHAR NOT NULL,
    PRIMARY KEY (organization_id, user_id)
);
