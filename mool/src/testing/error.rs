use thiserror::Error;

use crate::DbError;

/// Errors raised while provisioning, migrating, or removing a test database.
#[derive(Debug, Error)]
pub enum TestDatabaseError {
    /// The source configuration cannot be converted into an isolated target URL.
    #[error("cannot derive an isolated test database URL: {reason}")]
    TargetUrl {
        /// Concise explanation of the invalid URL shape.
        reason: String,
    },
    /// A temporary SQLite directory could not be created.
    #[error("cannot create a temporary SQLite test directory: {source}")]
    TemporaryDirectory {
        /// Filesystem error returned while provisioning the directory.
        #[source]
        source: std::io::Error,
    },
    /// The selected backend rejected creation of the isolated target.
    #[error("cannot create test database {target}: {source}")]
    Create {
        /// Generated database name or SQLite path.
        target: String,
        /// Database driver error.
        #[source]
        source: sqlx::Error,
    },
    /// Mool could not construct the pool for the isolated target.
    #[error("cannot open test database pool: {source}")]
    Pool {
        /// Pool construction error.
        #[source]
        source: DbError,
    },
    /// A migration-enabled test database requires an application root source.
    #[cfg(all(feature = "migrations", any(feature = "postgres", feature = "sqlite")))]
    #[error("cannot apply test migrations without a root migration source")]
    MissingMigrationRoot,
    /// Gaman could not apply the registered migration history.
    #[cfg(all(feature = "migrations", any(feature = "postgres", feature = "sqlite")))]
    #[error("cannot apply migrations to test database {target}: {source}")]
    Migrations {
        /// Generated database name or SQLite path.
        target: String,
        /// Structured migration runner failure.
        #[source]
        source: crate::migrations::engine::MigrationCommandError,
    },
    /// The isolated target could not be removed.
    #[error("cannot remove test database {target}: {source}")]
    Teardown {
        /// Generated database name or SQLite path.
        target: String,
        /// Database driver error.
        #[source]
        source: sqlx::Error,
    },
}
