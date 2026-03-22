#![cfg(not(tarpaulin_include))]

/// Models module
pub mod models;
/// Repository pattern implementations
/// Repository module
pub mod repository;
/// Diesel schema
/// Schema module
pub mod schema;

#[cfg(test)]
/// Tests for database models
pub mod tests;


use diesel::pg::PgConnection;
use diesel::r2d2::{self, ConnectionManager};

/// Database connection pool
pub type DbPool = r2d2::Pool<ConnectionManager<PgConnection>>;

/// Establish connection pool for PostgreSQL
pub fn establish_connection_pool(database_url: &str) -> DbPool {
    let manager = ConnectionManager::<PgConnection>::new(database_url);
    r2d2::Pool::builder()
        .build(manager)
        .expect("Failed to create pool.")
}
