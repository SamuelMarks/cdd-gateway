#![deny(missing_docs, clippy::missing_docs_in_private_items)]
//! Test harness and integration tests for `PostgreSQL` database repositories.

/// Organization repository integration tests.
#[cfg(test)]
pub mod org_tests;
/// Release repository integration tests.
#[cfg(test)]
pub mod release_tests;
/// Repository CRUD integration tests.
#[cfg(test)]
pub mod repo_tests;
/// User repository integration tests.
#[cfg(test)]
pub mod user_tests;

#[cfg(test)]
use crate::db::establish_connection_pool;
#[cfg(test)]
use crate::db::repository::PgRepository;
#[cfg(test)]
use crate::error::CddGatewayError;
#[cfg(test)]
use derive_more::Display;
#[cfg(test)]
use diesel_migrations::{embed_migrations, EmbeddedMigrations, MigrationHarness};

/// Embedded SQL migrations used to initialize the test database.
#[cfg(test)]
pub const MIGRATIONS: EmbeddedMigrations = embed_migrations!("migrations");

/// Error type used by database integration tests.
#[cfg(test)]
#[derive(Debug, Display)]
pub enum TestError {
    /// Gateway or database error.
    #[display("Gateway error: {_0}")]
    Gateway(CddGatewayError),
    /// Database error from Diesel.
    #[display("Database error: {_0}")]
    DbError(diesel::result::Error),
    /// Environment variable lookup error.
    #[display("Environment error: {_0}")]
    EnvError(std::env::VarError),
    /// System clock error.
    #[display("Time error: {_0}")]
    TimeError(std::time::SystemTimeError),
    /// Connection pool initialization error.
    #[display("Pool error: {_0}")]
    PoolError(String),
    /// Database migration execution error.
    #[display("Migration error: {_0}")]
    MigrationError(String),
    /// Expected `Some` value was `None`.
    #[display("None error")]
    NoneError,
}

#[cfg(test)]
impl From<CddGatewayError> for TestError {
    fn from(err: CddGatewayError) -> Self {
        Self::Gateway(err)
    }
}

#[cfg(test)]
impl From<diesel::result::Error> for TestError {
    fn from(err: diesel::result::Error) -> Self {
        Self::DbError(err)
    }
}

#[cfg(test)]
impl From<std::env::VarError> for TestError {
    fn from(err: std::env::VarError) -> Self {
        Self::EnvError(err)
    }
}

#[cfg(test)]
impl From<std::time::SystemTimeError> for TestError {
    fn from(err: std::time::SystemTimeError) -> Self {
        Self::TimeError(err)
    }
}

#[cfg(test)]
impl std::error::Error for TestError {}

/// Initializes the test `PostgreSQL` database pool and runs pending migrations once.
///
/// # Returns
/// * `Ok(PgRepository)` - Connected repository ready for integration tests.
///
/// # Errors
/// * [`TestError`] - If establishing the pool or running migrations fails.
#[cfg(test)]
pub fn setup_test_db() -> Result<PgRepository, TestError> {
    static MIGRATION_RESULT: std::sync::OnceLock<Result<(), String>> = std::sync::OnceLock::new();

    let database_url = std::env::var("DATABASE_URL")
        .unwrap_or_else(|_| "postgres://postgres:password@localhost/cdd".to_string());
    let pool = establish_connection_pool(&database_url)
        .map_err(|e| TestError::PoolError(e.to_string()))?;

    let result = MIGRATION_RESULT.get_or_init(|| {
        let mut conn = pool.get().map_err(|e| e.to_string())?;
        if let Err(e) = conn.run_pending_migrations(MIGRATIONS) {
            log::warn!("Migration warning: {e}");
        }
        Ok(())
    });

    if let Err(e) = result {
        return Err(TestError::MigrationError(e.clone()));
    }

    Ok(PgRepository { pool })
}
