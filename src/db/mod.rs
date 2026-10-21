#![deny(missing_docs, clippy::missing_docs_in_private_items)]
//! Database connection pool management and module exports.
//!
//! Exposes [`models`], [`repository`], and [`schema`] submodules along with
//! [`establish_connection_pool`] for initializing `PostgreSQL` connection pools.

/// Database entity models and domain types.
pub mod models;
/// Repository pattern traits and implementations (`PgRepository` and `OfflineRepository`).
pub mod repository;
/// Auto-generated Diesel table schema definitions.
pub mod schema;

#[cfg(test)]
/// Integration and unit tests for database repositories and models.
pub mod tests;

use diesel::pg::PgConnection;
use diesel::r2d2::{self, ConnectionManager};

/// Type alias for the `r2d2` `PostgreSQL` connection pool.
pub type DbPool = r2d2::Pool<ConnectionManager<PgConnection>>;

/// Establishes an `r2d2` connection pool for `PostgreSQL`.
///
/// # Arguments
/// * `database_url` - The `PostgreSQL` connection string (e.g., `"postgres://user:pass@host/db"`).
///
/// # Returns
/// * `Ok(DbPool)` - The initialized connection pool.
///
/// # Errors
/// * [`crate::error::CddGatewayError::Pool`] - If the pool fails to initialize or connect.
pub fn establish_connection_pool(
    database_url: &str,
) -> Result<DbPool, crate::error::CddGatewayError> {
    let manager = ConnectionManager::<PgConnection>::new(database_url);
    r2d2::Pool::builder()
        .build(manager)
        .map_err(crate::error::CddGatewayError::Pool)
}

#[cfg(test)]
mod connection_tests {
    use super::*;

    #[test]
    fn test_establish_connection_pool_invalid_url() {
        let result = establish_connection_pool("invalid_url");
        assert!(result.is_err());
        if let Err(err) = result {
            assert_eq!(
                actix_web::ResponseError::status_code(&err),
                actix_web::http::StatusCode::INTERNAL_SERVER_ERROR
            );
        }
    }
}
