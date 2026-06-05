#![allow(missing_docs)]
#[cfg(test)]
pub mod org_tests;
#[cfg(test)]
pub mod release_tests;
#[cfg(test)]
pub mod repo_tests;
#[cfg(test)]
pub mod user_tests;

#[cfg(test)]
use crate::db::establish_connection_pool;
#[cfg(test)]
use crate::db::repository::PgRepository;
#[cfg(test)]
use derive_more::Display;
#[cfg(test)]
use diesel_migrations::{embed_migrations, EmbeddedMigrations, MigrationHarness};

#[cfg(test)]
pub const MIGRATIONS: EmbeddedMigrations = embed_migrations!("migrations");

#[cfg(test)]
#[derive(Debug, Display)]
pub enum TestError {
    #[display("Database error: {_0}")]
    DbError(diesel::result::Error),
    #[display("Environment error: {_0}")]
    EnvError(std::env::VarError),
    #[display("Time error: {_0}")]
    TimeError(std::time::SystemTimeError),
    #[display("Pool error: {_0}")]
    PoolError(String),
    #[display("Migration error: {_0}")]
    MigrationError(String),
    #[display("Not found: {_0}")]
    NotFound(String),
    #[display("None error")]
    NoneError,
}

#[cfg(test)]
impl From<diesel::result::Error> for TestError {
    fn from(err: diesel::result::Error) -> Self {
        TestError::DbError(err)
    }
}

#[cfg(test)]
impl From<std::env::VarError> for TestError {
    fn from(err: std::env::VarError) -> Self {
        TestError::EnvError(err)
    }
}

#[cfg(test)]
impl From<std::time::SystemTimeError> for TestError {
    fn from(err: std::time::SystemTimeError) -> Self {
        TestError::TimeError(err)
    }
}

#[cfg(test)]
impl std::error::Error for TestError {}

#[cfg(test)]
pub fn setup_test_db() -> Result<PgRepository, TestError> {
    static MIGRATION_RESULT: std::sync::OnceLock<Result<(), String>> = std::sync::OnceLock::new();

    let database_url = std::env::var("DATABASE_URL")
        .unwrap_or_else(|_| "postgres://postgres:password@localhost/cdd".to_string());
    let pool = establish_connection_pool(&database_url);

    let result = MIGRATION_RESULT.get_or_init(|| {
        let mut conn = pool.get().map_err(|e| e.to_string())?;
        if let Err(e) = conn.run_pending_migrations(MIGRATIONS) {
            log::warn!("Migration warning: {}", e);
        }
        Ok(())
    });

    if let Err(e) = result {
        return Err(TestError::MigrationError(e.clone()));
    }

    Ok(PgRepository { pool })
}
