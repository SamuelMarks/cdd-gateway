// @generated automatically by Diesel CLI.

diesel::table! {
    /// Organizations (Projects) mapping.
    organizations (id) {
        /// Internal DB ID
        id -> Int4,
        /// GitHub ID, if synced
        github_id -> Nullable<Int8>,
        /// Login/Name
        login -> Varchar,
        /// Description
        description -> Nullable<Text>,
    }
}

diesel::table! {
    /// Organization Members mapping RBAC roles
    organization_users (organization_id, user_id) {
        /// Organization ID
        organization_id -> Int4,
        /// User ID
        user_id -> Int4,
        /// Role (e.g., "owner", "member")
        role -> Varchar,
    }
}

diesel::table! {
    /// Releases mapping
    releases (id) {
        /// Internal DB ID
        id -> Int4,
        /// Repository ID
        repository_id -> Int4,
        /// GitHub ID, if synced
        github_id -> Nullable<Int8>,
        /// Tag Name
        tag_name -> Varchar,
        /// Name
        name -> Nullable<Varchar>,
        /// Body
        body -> Nullable<Text>,
    }
}

diesel::table! {
    /// Repositories (SDKs) mapping
    repositories (id) {
        /// Internal DB ID
        id -> Int4,
        /// Organization ID
        organization_id -> Int4,
        /// GitHub ID, if synced
        github_id -> Nullable<Int8>,
        /// Name
        name -> Varchar,
        /// Description
        description -> Nullable<Text>,
    }
}

diesel::table! {
    /// Users mapping
    users (id) {
        /// Internal DB ID
        id -> Int4,
        /// GitHub ID, if synced
        github_id -> Nullable<Int8>,
        /// Username
        username -> Varchar,
        /// Email
        email -> Varchar,
        /// Password Hash
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
