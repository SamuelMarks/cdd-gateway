pub mod models;
pub mod repository;
pub mod schema;

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
