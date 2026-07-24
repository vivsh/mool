//! Compatibility types for applications compiled without a database backend.

use std::marker::PhantomData;

use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Marker database type used when Mool is compiled without a backend feature.
#[derive(Debug, Clone, Copy, Default)]
pub struct Database;

/// Marker argument container used when Mool is compiled without a backend feature.
#[derive(Debug, Clone, Default)]
pub struct Arguments<'q>(PhantomData<&'q ()>);

/// Marker row type used when Mool is compiled without a backend feature.
#[derive(Debug, Clone, Default)]
pub struct Row;

/// Marker query-result type used when Mool is compiled without a backend feature.
#[derive(Debug, Clone, Copy, Default)]
pub struct QueryResult;

/// Inert pool handle used when Mool is compiled without a backend feature.
#[derive(Debug, Clone, Default)]
pub struct Pool;

/// Database integrity constraint category.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum IntegrityKind {
    /// Unique constraint violation.
    Unique,
    /// Foreign-key constraint violation.
    ForeignKey,
    /// Check constraint violation.
    Check,
    /// Non-null constraint violation.
    NotNull,
    /// PostgreSQL exclusion constraint violation.
    Exclusion,
    /// Backend integrity category not recognized by Mool.
    Other(String),
}

/// Database operation associated with an error.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DbOperation {
    /// Establishing a database connection.
    Connect,
    /// Beginning a transaction.
    Begin,
    /// Beginning a nested transaction or savepoint.
    Savepoint,
    /// Committing a transaction.
    Commit,
    /// Rolling back a transaction or savepoint.
    Rollback,
    /// Executing a statement that does not return rows.
    Execute,
    /// Fetching exactly one row.
    FetchOne,
    /// Fetching all rows.
    FetchAll,
    /// Fetching an optional row.
    FetchOptional,
    /// Fetching one scalar value.
    FetchScalar,
    /// Operation context was not supplied by the caller.
    Unknown,
}

impl std::fmt::Display for DbOperation {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "{self:?}")
    }
}

impl std::fmt::Display for IntegrityKind {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Unique => formatter.write_str("unique"),
            Self::ForeignKey => formatter.write_str("foreign key"),
            Self::Check => formatter.write_str("check"),
            Self::NotNull => formatter.write_str("not null"),
            Self::Exclusion => formatter.write_str("exclusion"),
            Self::Other(code) => write!(formatter, "other ({code})"),
        }
    }
}

/// Query failure returned when a database operation is attempted without a backend.
#[derive(Debug, Clone, Error)]
#[error("query execution requires one Mool backend feature")]
pub struct BackendlessQueryError;

/// Compatibility query-error type for applications compiled without a backend.
pub type QueryError = BackendlessQueryError;

/// Error returned by the backendless compatibility surface.
#[derive(Debug, Error)]
pub enum DbError {
    /// A database constraint rejected the operation.
    #[error("{operation} failed with an integrity violation: {kind}")]
    Integrity {
        /// Database operation that failed.
        operation: DbOperation,
        /// Constraint category.
        kind: IntegrityKind,
        /// Optional constraint name.
        constraint: Option<String>,
        /// Original SQLx error.
        #[source]
        source: sqlx::Error,
    },
    /// Query returned multiple rows where at most one was required.
    #[error("query returned multiple rows")]
    MultipleObjects,
    /// Query did not return the required row.
    #[error("record not found")]
    DoesNotExist,
    /// A query operation was attempted without a selected Mool backend.
    #[error("query planning failed: {0}")]
    QuerySet(#[from] QueryError),
    /// A database connection could not be established or retained.
    #[error("{operation} failed because the database connection is unavailable")]
    Connection {
        /// Database operation that failed.
        operation: DbOperation,
        /// Original SQLx error.
        #[source]
        source: sqlx::Error,
    },
    /// A database operation timed out.
    #[error("{operation} timed out")]
    Timeout {
        /// Database operation that timed out.
        operation: DbOperation,
        /// Original SQLx error.
        #[source]
        source: sqlx::Error,
    },
    /// A serialization conflict aborted the database operation.
    #[error("{operation} failed due to a serialization conflict")]
    Serialization {
        /// Database operation that failed.
        operation: DbOperation,
        /// Original SQLx error.
        #[source]
        source: sqlx::Error,
    },
    /// A database deadlock aborted the operation.
    #[error("{operation} failed due to a deadlock")]
    Deadlock {
        /// Database operation that failed.
        operation: DbOperation,
        /// Original SQLx error.
        #[source]
        source: sqlx::Error,
    },
    /// The database cancelled the operation.
    #[error("{operation} was cancelled")]
    Cancelled {
        /// Database operation that failed.
        operation: DbOperation,
        /// Original SQLx error.
        #[source]
        source: sqlx::Error,
    },
    /// A database value could not be decoded.
    #[error("{operation} could not decode the database result")]
    Decode {
        /// Database operation that failed.
        operation: DbOperation,
        /// Original SQLx error.
        #[source]
        source: sqlx::Error,
    },
    /// Database configuration is invalid.
    #[error("invalid database configuration during {operation}")]
    Configuration {
        /// Database operation that failed.
        operation: DbOperation,
        /// Original SQLx error.
        #[source]
        source: sqlx::Error,
    },
    /// The backend returned an unclassified database error.
    #[error("{operation} failed with database error {code:?}: {message}")]
    Database {
        /// Database operation that failed.
        operation: DbOperation,
        /// Optional database error code.
        code: Option<String>,
        /// Database error message.
        message: String,
        /// Original SQLx error.
        #[source]
        source: sqlx::Error,
    },
    /// SQLx returned an unclassified error.
    #[error("{operation} failed with an unclassified SQLx error")]
    Sqlx {
        /// Database operation that failed.
        operation: DbOperation,
        /// Original SQLx error.
        #[source]
        source: sqlx::Error,
    },
    /// SQLite is busy.
    #[error("{operation} could not proceed because SQLite is busy")]
    Busy {
        /// Database operation that failed.
        operation: DbOperation,
        /// Original SQLx error.
        #[source]
        source: sqlx::Error,
    },
    /// SQLite is locked.
    #[error("{operation} could not proceed because SQLite is locked")]
    Locked {
        /// Database operation that failed.
        operation: DbOperation,
        /// Original SQLx error.
        #[source]
        source: sqlx::Error,
    },
    /// A database operation was attempted without a selected Mool backend.
    #[error("backend capability '{capability}' is unavailable during {operation}")]
    Capability {
        /// Operation requested by the caller.
        operation: &'static str,
        /// Required backend capability.
        capability: &'static str,
    },
    /// A mock session could not perform the requested operation.
    #[error("mock {operation} failed: {reason}")]
    Mock {
        /// Mock operation that failed.
        operation: &'static str,
        /// Failure reason.
        reason: String,
    },
    /// A batch affected-row count overflowed.
    #[error("affected-row count overflowed u64 while combining batch chunks")]
    AffectedRowsOverflow,
    /// A relation definition is invalid.
    #[error("relation '{relation}' is invalid: {reason}")]
    Relation {
        /// Relation name.
        relation: &'static str,
        /// Failure reason.
        reason: String,
    },
}

impl DbError {
    /// Returns a stable machine-readable category for this database error.
    pub const fn code(&self) -> &'static str {
        match self {
            Self::Integrity { .. } => "integrity_violation",
            Self::MultipleObjects => "multiple_objects",
            Self::DoesNotExist => "not_found",
            Self::QuerySet(_) => "statement_error",
            Self::Connection { .. } => "connection_error",
            Self::Timeout { .. } => "timeout",
            Self::Serialization { .. } => "serialization_failure",
            Self::Deadlock { .. } => "deadlock",
            Self::Cancelled { .. } => "cancelled",
            Self::Decode { .. } => "decode_error",
            Self::Configuration { .. } => "configuration_error",
            Self::Database { .. } => "database_error",
            Self::Sqlx { .. } => "sqlx_error",
            Self::Busy { .. } => "database_busy",
            Self::Locked { .. } => "database_locked",
            Self::Capability { .. } => "unsupported_feature",
            Self::Mock { .. } => "mock_error",
            Self::AffectedRowsOverflow => "affected_rows_overflow",
            Self::Relation { .. } => "relation_error",
        }
    }

    /// Wraps an SQLx error when no selected backend is available for classification.
    pub fn from_sqlx(operation: DbOperation, source: sqlx::Error) -> Self {
        Self::Sqlx { operation, source }
    }
}

impl From<sqlx::Error> for DbError {
    fn from(source: sqlx::Error) -> Self {
        Self::from_sqlx(DbOperation::Unknown, source)
    }
}

/// Database-pool configuration retained by backendless applications.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DbConf {
    /// SQLx URL used when an application selects a Mool backend.
    pub url: String,
    /// Connections opened eagerly when a database backend is active.
    pub min_connections: u32,
    /// Maximum concurrent connections when a database backend is active.
    pub max_connections: u32,
    /// Whether connection establishment is deferred when a database backend is active.
    pub lazy: bool,
}

impl Default for DbConf {
    fn default() -> Self {
        Self {
            url: String::new(),
            min_connections: 0,
            max_connections: 0,
            lazy: true,
        }
    }
}

impl DbConf {
    /// Loads a backend configuration from `DATABASE_URL`.
    pub fn from_env() -> Result<Self, DbError> {
        let url = std::env::var("DATABASE_URL")
            .map_err(|_| configuration_error("DATABASE_URL not set"))?;
        Self::from_url(&url)
    }

    /// Parses a database URL without selecting or opening a backend.
    pub fn from_url(url: &str) -> Result<Self, DbError> {
        let mut parsed = url::Url::parse(url).map_err(|_| configuration_error("invalid URL"))?;
        let mut max_connections = 10;
        let mut min_connections = 1;
        let mut lazy = false;
        let mut retained = Vec::new();

        for (name, value) in parsed.query_pairs() {
            match name.as_ref() {
                "max" => max_connections = parse_pool_count("max", &value)?,
                "min" => min_connections = parse_pool_count("min", &value)?,
                "lazy" => lazy = parse_lazy(&value)?,
                _ => retained.push((name.into_owned(), value.into_owned())),
            }
        }

        if min_connections > max_connections {
            return Err(configuration_error("min must not exceed max"));
        }
        parsed.query_pairs_mut().clear().extend_pairs(retained);
        Ok(Self {
            url: parsed.into(),
            min_connections,
            max_connections,
            lazy,
        })
    }
}

/// Parses one Mool pool-size parameter.
fn parse_pool_count(name: &str, value: &str) -> Result<u32, DbError> {
    let count = value
        .parse::<u32>()
        .map_err(|_| configuration_error(&format!("{name} must be an unsigned integer")))?;
    if name == "max" && count == 0 {
        return Err(configuration_error("max must be greater than zero"));
    }
    Ok(count)
}

/// Parses Mool's lazy-pool option.
fn parse_lazy(value: &str) -> Result<bool, DbError> {
    value
        .parse::<bool>()
        .map_err(|_| configuration_error("lazy must be true or false"))
}

/// Creates a configuration error without selecting an SQLx backend.
fn configuration_error(reason: &str) -> DbError {
    DbError::from_sqlx(
        DbOperation::Connect,
        sqlx::Error::InvalidArgument(reason.to_string()),
    )
}

/// Pool facade retained for framework memory-only builds.
#[derive(Debug, Clone, Default)]
pub struct DbPool {
    configured: bool,
}

impl DbPool {
    /// Wraps the backendless marker pool for API-compatible framework construction.
    pub fn from_pool(_pool: Pool) -> Self {
        Self { configured: true }
    }

    /// Creates a disabled pool for an empty memory-only configuration.
    ///
    /// A non-empty URL requires rebuilding Mool with exactly one backend feature.
    pub async fn from_conf(conf: &DbConf) -> Result<Self, DbError> {
        if conf.url.is_empty() {
            return Ok(Self { configured: false });
        }
        Err(DbError::Capability {
            operation: "DbPool::from_conf",
            capability: "a selected Mool backend",
        })
    }

    /// Returns whether this compatibility pool was supplied by a caller.
    pub const fn is_configured(&self) -> bool {
        self.configured
    }
}
