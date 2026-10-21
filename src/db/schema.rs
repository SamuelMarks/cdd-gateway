#![deny(missing_docs, clippy::missing_docs_in_private_items)]
//! Auto-generated Diesel database schema definitions.
//!
//! Defines the SQL table mappings (`users`, `organizations`, `organization_users`,
//! `repositories`, and `releases`) and their join relationships.

diesel::table! {
    /// Join table linking organizations and users with a role.
    organization_users (organization_id, user_id) {
        /// Foreign key referencing `organizations.id`.
        organization_id -> Int4,
        /// Foreign key referencing `users.id`.
        user_id -> Int4,
        /// Role assigned to the user within the organization.
        role -> Varchar,
    }
}

diesel::table! {
    /// Table storing organizations.
    organizations (id) {
        /// Primary key of the organization.
        id -> Int4,
        /// Optional GitHub organization ID.
        github_id -> Nullable<Int8>,
        /// Unique organization login name.
        login -> Varchar,
        /// Optional description of the organization.
        description -> Nullable<Text>,
    }
}

diesel::table! {
    /// Table storing repository releases.
    releases (id) {
        /// Primary key of the release.
        id -> Int4,
        /// Foreign key referencing `repositories.id`.
        repository_id -> Int4,
        /// Optional GitHub release ID.
        github_id -> Nullable<Int8>,
        /// Release Git tag name.
        tag_name -> Varchar,
        /// Optional release title.
        name -> Nullable<Varchar>,
        /// Optional release notes body.
        body -> Nullable<Text>,
    }
}

diesel::table! {
    /// Table storing repositories belonging to organizations.
    repositories (id) {
        /// Primary key of the repository.
        id -> Int4,
        /// Foreign key referencing `organizations.id`.
        organization_id -> Int4,
        /// Optional GitHub repository ID.
        github_id -> Nullable<Int8>,
        /// Repository name.
        name -> Varchar,
        /// Optional repository description.
        description -> Nullable<Text>,
    }
}

diesel::table! {
    /// Table storing registered or synced users.
    users (id) {
        /// Primary key of the user.
        id -> Int4,
        /// Optional GitHub user ID.
        github_id -> Nullable<Int8>,
        /// Unique username.
        username -> Varchar,
        /// User email address.
        email -> Varchar,
        /// Optional Argon2 password hash.
        password_hash -> Nullable<Varchar>,
    }
}

diesel::joinable!(organization_users -> organizations (organization_id));
diesel::joinable!(organization_users -> users (user_id));
diesel::joinable!(releases -> repositories (repository_id));
diesel::joinable!(repositories -> organizations (organization_id));

diesel::allow_tables_to_appear_in_same_query!(
    organization_users,
    organizations,
    releases,
    repositories,
    users,
);
