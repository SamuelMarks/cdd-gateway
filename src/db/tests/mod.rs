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
use diesel_migrations::{embed_migrations, EmbeddedMigrations, MigrationHarness};
#[cfg(test)]
use std::sync::Once;

/// Embedded Diesel SQL migrations applied once per test process.
#[cfg(test)]
pub const MIGRATIONS: EmbeddedMigrations = embed_migrations!("migrations");

/// Synchronization primitive ensuring database migrations run at most once per test binary.
#[cfg(test)]
static MIGRATION_ONCE: Once = Once::new();

/// Establishes a connection pool to the test database and runs pending migrations once.
///
/// # Returns
/// * `Ok(PgRepository)` - Connected repository backed by the test `PostgreSQL` database.
///
/// # Errors
/// * [`CddGatewayError`] - If connecting or running migrations fails.
#[cfg(test)]
pub fn setup_test_db() -> Result<PgRepository, CddGatewayError> {
    let database_url = std::env::var("DATABASE_URL")
        .unwrap_or_else(|_| "postgres://postgres:password@localhost/cdd".to_string());
    let pool = establish_connection_pool(&database_url)?;

    MIGRATION_ONCE.call_once(|| {
        if let Ok(mut conn) = pool.get() {
            let _ = conn.run_pending_migrations(MIGRATIONS);
        }
    });

    Ok(PgRepository { pool })
}
