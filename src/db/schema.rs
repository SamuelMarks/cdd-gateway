#![allow(missing_docs)]
#![cfg(not(tarpaulin_include))]

// @generated automatically by Diesel CLI.

diesel::table! {
    organization_users (organization_id, user_id) {
        organization_id -> Int4,
        user_id -> Int4,
        role -> Varchar,
    }
}

diesel::table! {
    organizations (id) {
        id -> Int4,
        github_id -> Nullable<Int8>,
        login -> Varchar,
        description -> Nullable<Text>,
    }
}

diesel::table! {
    releases (id) {
        id -> Int4,
        repository_id -> Int4,
        github_id -> Nullable<Int8>,
        tag_name -> Varchar,
        name -> Nullable<Varchar>,
        body -> Nullable<Text>,
    }
}

diesel::table! {
    repositories (id) {
        id -> Int4,
        organization_id -> Int4,
        github_id -> Nullable<Int8>,
        name -> Varchar,
        description -> Nullable<Text>,
    }
}

diesel::table! {
    users (id) {
        id -> Int4,
        github_id -> Nullable<Int8>,
        username -> Varchar,
        email -> Varchar,
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
