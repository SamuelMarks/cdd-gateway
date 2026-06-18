/// Models module
pub mod models;
/// Repository pattern implementations
/// Repository module
pub mod repository;
/// Diesel schema
/// Schema module
#[allow(missing_docs)]
pub mod schema;

#[cfg(test)]
/// Tests for database models
pub mod tests;

use diesel::pg::PgConnection;
use diesel::r2d2::{self, ConnectionManager};

/// Database connection pool
pub type DbPool = r2d2::Pool<ConnectionManager<PgConnection>>;

/// Establish connection pool for `PostgreSQL`
/// # Errors
/// error
pub fn establish_connection_pool(
    database_url: &str,
) -> Result<DbPool, crate::error::CddGatewayError> {
    let manager = ConnectionManager::<PgConnection>::new(database_url);
    r2d2::Pool::builder()
        .build(manager)
        .map_err(|e| crate::error::CddGatewayError::Internal(e.to_string()))
}

#[cfg(test)]
mod connection_tests {
    use super::*;

    #[test]
    fn test_establish_connection_pool_invalid_url() {
        let result = establish_connection_pool("invalid_url");
        assert!(result.is_err());
    }
}
