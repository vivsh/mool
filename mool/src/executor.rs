#![allow(async_fn_in_trait)]

use crate::backend::{Database, Pool, Row};
use crate::{QueryError, Statement};
use serde::{Deserialize, Serialize};
use std::future::Future;
use std::pin::Pin;
#[cfg(feature = "tracing")]
use std::{hash::Hasher, time::Instant};
use thiserror::Error;

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

/// Operation that was in progress when SQLx returned an error.
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
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:?}", self)
    }
}

impl std::fmt::Display for IntegrityKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Unique => f.write_str("unique"),
            Self::ForeignKey => f.write_str("foreign key"),
            Self::Check => f.write_str("check"),
            Self::NotNull => f.write_str("not null"),
            Self::Exclusion => f.write_str("exclusion"),
            Self::Other(code) => write!(f, "other ({code})"),
        }
    }
}

#[derive(Debug, Error)]
pub enum DbError {
    #[error("{operation} failed with an integrity violation: {kind}")]
    Integrity {
        operation: DbOperation,
        kind: IntegrityKind,
        constraint: Option<String>,
        #[source]
        source: sqlx::Error,
    },
    #[error("query returned multiple rows")]
    MultipleObjects,
    #[error("record not found")]
    DoesNotExist,
    #[error("query planning failed: {0}")]
    QuerySet(#[from] QueryError),
    #[error("{operation} failed because the database connection is unavailable")]
    Connection {
        operation: DbOperation,
        #[source]
        source: sqlx::Error,
    },
    #[error("{operation} timed out")]
    Timeout {
        operation: DbOperation,
        #[source]
        source: sqlx::Error,
    },
    #[error("{operation} failed due to a serialization conflict")]
    Serialization {
        operation: DbOperation,
        #[source]
        source: sqlx::Error,
    },
    #[error("{operation} failed due to a deadlock")]
    Deadlock {
        operation: DbOperation,
        #[source]
        source: sqlx::Error,
    },
    #[error("{operation} was cancelled")]
    Cancelled {
        operation: DbOperation,
        #[source]
        source: sqlx::Error,
    },
    #[error("{operation} could not decode the database result")]
    Decode {
        operation: DbOperation,
        #[source]
        source: sqlx::Error,
    },
    #[error("invalid database configuration during {operation}")]
    Configuration {
        operation: DbOperation,
        #[source]
        source: sqlx::Error,
    },
    #[error("{operation} failed with database error {code}: {message}")]
    Database {
        operation: DbOperation,
        code: String,
        message: String,
        #[source]
        source: sqlx::Error,
    },
    #[error("{operation} failed with an unclassified SQLx error")]
    Sqlx {
        operation: DbOperation,
        #[source]
        source: sqlx::Error,
    },
    #[error("backend capability '{capability}' is unavailable during {operation}")]
    Capability {
        operation: &'static str,
        capability: &'static str,
    },
    #[error("mock {operation} failed: {reason}")]
    Mock {
        operation: &'static str,
        reason: String,
    },
    #[error("rollback failed after an earlier transaction error: {original}")]
    RollbackFailed {
        original: Box<DbError>,
        rollback: Box<DbError>,
    },
    #[error("affected-row count overflowed u64 while combining batch chunks")]
    AffectedRowsOverflow,
    #[error("relation '{relation}' is invalid: {reason}")]
    Relation {
        relation: &'static str,
        reason: String,
    },
}

impl DbError {
    /// Returns a stable machine-readable category for this database error.
    pub const fn code(&self) -> &'static str {
        match self {
            DbError::Integrity { .. } => "integrity_violation",
            DbError::MultipleObjects => "multiple_objects",
            DbError::DoesNotExist => "not_found",
            DbError::QuerySet(_) => "statement_error",
            DbError::Connection { .. } => "connection_error",
            DbError::Timeout { .. } => "timeout",
            DbError::Serialization { .. } => "serialization_failure",
            DbError::Deadlock { .. } => "deadlock",
            DbError::Cancelled { .. } => "cancelled",
            DbError::Decode { .. } => "decode_error",
            DbError::Configuration { .. } => "configuration_error",
            DbError::Database { .. } => "database_error",
            DbError::Sqlx { .. } => "sqlx_error",
            DbError::Capability { .. } => "unsupported_feature",
            DbError::Mock { .. } => "mock_error",
            DbError::RollbackFailed { .. } => "rollback_failed",
            DbError::AffectedRowsOverflow => "affected_rows_overflow",
            DbError::Relation { .. } => "relation_error",
        }
    }

    /// Classifies an SQLx error while retaining its operation context and source.
    pub fn from_sqlx(operation: DbOperation, error: sqlx::Error) -> Self {
        if matches!(error, sqlx::Error::RowNotFound) {
            return Self::DoesNotExist;
        }
        if let Some(metadata) = database_error_metadata(&error) {
            return classify_database_error(operation, metadata, error);
        }
        match error {
            error @ (sqlx::Error::Io(_)
            | sqlx::Error::Tls(_)
            | sqlx::Error::PoolClosed
            | sqlx::Error::WorkerCrashed) => Self::Connection {
                operation,
                source: error,
            },
            error @ sqlx::Error::PoolTimedOut => Self::Timeout {
                operation,
                source: error,
            },
            error @ (sqlx::Error::ColumnIndexOutOfBounds { .. }
            | sqlx::Error::ColumnNotFound(_)
            | sqlx::Error::ColumnDecode { .. }
            | sqlx::Error::Decode(_)) => Self::Decode {
                operation,
                source: error,
            },
            error @ (sqlx::Error::Configuration(_) | sqlx::Error::InvalidArgument(_)) => {
                Self::Configuration {
                    operation,
                    source: error,
                }
            }
            source => Self::Sqlx { operation, source },
        }
    }
}

impl From<sqlx::Error> for DbError {
    fn from(error: sqlx::Error) -> Self {
        Self::from_sqlx(DbOperation::Unknown, error)
    }
}

fn classify_database_error(
    operation: DbOperation,
    metadata: DatabaseErrorMetadata,
    source: sqlx::Error,
) -> DbError {
    if let Some(kind) = metadata.integrity {
        return DbError::Integrity {
            operation,
            kind,
            constraint: metadata.constraint,
            source,
        };
    }
    let code = metadata.code;
    if is_serialization(&code) {
        return DbError::Serialization { operation, source };
    }
    if is_deadlock(&code) {
        return DbError::Deadlock { operation, source };
    }
    if is_timeout(&code) {
        return DbError::Timeout { operation, source };
    }
    if is_cancelled(&code) {
        return DbError::Cancelled { operation, source };
    }
    DbError::Database {
        operation,
        code,
        message: metadata.message,
        source,
    }
}

struct DatabaseErrorMetadata {
    integrity: Option<IntegrityKind>,
    constraint: Option<String>,
    code: String,
    message: String,
}

fn database_error_metadata(error: &sqlx::Error) -> Option<DatabaseErrorMetadata> {
    let sqlx::Error::Database(database) = error else {
        return None;
    };
    Some(DatabaseErrorMetadata {
        integrity: integrity_kind(database.as_ref()),
        constraint: database.constraint().map(str::to_owned),
        code: database.code().unwrap_or_default().into_owned(),
        message: database.message().to_owned(),
    })
}

fn integrity_kind(database: &dyn sqlx::error::DatabaseError) -> Option<IntegrityKind> {
    match database.kind() {
        sqlx::error::ErrorKind::UniqueViolation => Some(IntegrityKind::Unique),
        sqlx::error::ErrorKind::ForeignKeyViolation => Some(IntegrityKind::ForeignKey),
        sqlx::error::ErrorKind::NotNullViolation => Some(IntegrityKind::NotNull),
        sqlx::error::ErrorKind::CheckViolation => Some(IntegrityKind::Check),
        sqlx::error::ErrorKind::Other => None,
        _ => None,
    }
}

fn is_serialization(code: &str) -> bool {
    code == "40001"
}

fn is_deadlock(code: &str) -> bool {
    matches!(code, "40P01" | "1213")
}

fn is_timeout(code: &str) -> bool {
    matches!(code, "1205" | "5" | "6")
}

fn is_cancelled(code: &str) -> bool {
    matches!(code, "57014" | "1317")
}

pub trait DbSession {
    /// Executes a statement and returns its affected-row count.
    async fn execute(&mut self, qs: Statement) -> Result<u64, DbError>;

    /// Fetches exactly one decoded row.
    async fn fetch_one<M>(&mut self, qs: Statement) -> Result<M, DbError>
    where
        M: for<'r> sqlx::FromRow<'r, Row> + Send + Unpin + 'static;

    /// Fetches all decoded rows into memory.
    async fn fetch_all<M>(&mut self, qs: Statement) -> Result<Vec<M>, DbError>
    where
        M: for<'r> sqlx::FromRow<'r, Row> + Send + Unpin + 'static;

    /// Fetches zero or one decoded row.
    async fn fetch_optional<M>(&mut self, qs: Statement) -> Result<Option<M>, DbError>
    where
        M: for<'r> sqlx::FromRow<'r, Row> + Send + Unpin + 'static;

    /// Fetches exactly one scalar value.
    async fn fetch_scalar<T>(&mut self, qs: Statement) -> Result<T, DbError>
    where
        for<'d> T: sqlx::Decode<'d, Database> + sqlx::Type<Database> + Send + Unpin + 'static;
}

/// SQLx-backed transaction implementing [`DbSession`].
pub struct DbTransaction<'a> {
    transaction: sqlx::Transaction<'a, Database>,
}

/// Boxed asynchronous callback result used by [`DbPool::transaction`].
pub type TransactionFuture<'a, T> = Pin<Box<dyn Future<Output = Result<T, DbError>> + Send + 'a>>;

impl<'a> DbTransaction<'a> {
    /// Returns the underlying SQLx transaction for advanced backend operations.
    pub fn as_sqlx(&mut self) -> &mut sqlx::Transaction<'a, Database> {
        &mut self.transaction
    }

    /// Commits this transaction and consumes the handle.
    pub async fn commit(self) -> Result<(), DbError> {
        self.transaction
            .commit()
            .await
            .map_err(|error| DbError::from_sqlx(DbOperation::Commit, error))
    }

    /// Rolls back this transaction and consumes the handle.
    pub async fn rollback(self) -> Result<(), DbError> {
        self.transaction
            .rollback()
            .await
            .map_err(|error| DbError::from_sqlx(DbOperation::Rollback, error))
    }

    /// Starts a nested transaction backed by a database savepoint.
    pub async fn begin_nested(&mut self) -> Result<DbTransaction<'_>, DbError> {
        let transaction = sqlx::Acquire::begin(&mut *self.transaction)
            .await
            .map_err(|error| DbError::from_sqlx(DbOperation::Savepoint, error))?;
        Ok(DbTransaction { transaction })
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
/// Database pool configuration for the selected backend.
pub struct DbConf {
    /// SQLx database URL, including backend transport options.
    pub url: String,
    /// Connections opened eagerly when the pool starts.
    pub min_connections: u32,
    /// Maximum concurrent pool connections.
    pub max_connections: u32,
    /// Defers the first physical connection until the pool is used.
    pub lazy: bool,
}

impl Default for DbConf {
    /// Default configuration is always valid and zero-cost until first use.
    /// Uses feature-dependent URLs: sqlite::memory, postgres://localhost/test, or mysql://localhost/test
    fn default() -> Self {
        #[cfg(feature = "sqlite")]
        let url = "sqlite::memory:";

        #[cfg(feature = "postgres")]
        let url = "postgres://localhost/test";

        #[cfg(feature = "mysql")]
        let url = "mysql://localhost/test";

        #[cfg(feature = "mariadb")]
        let url = "mysql://localhost/test";

        Self {
            url: url.into(),
            min_connections: 0,
            max_connections: 5,
            lazy: true,
        }
    }
}

impl DbConf {
    /// Load configuration from DATABASE_URL environment variable.
    /// Supports query parameters: max, min, lazy
    /// Example: postgres://user:pass@host/db?max=20&min=2&lazy=true
    pub fn from_env() -> Result<Self, DbError> {
        let url = std::env::var("DATABASE_URL").map_err(|_| {
            DbError::from_sqlx(
                DbOperation::Connect,
                sqlx::Error::Configuration("DATABASE_URL not set".into()),
            )
        })?;

        Self::from_url(&url)
    }

    /// Parses configuration from a database URL string.
    ///
    /// Mool consumes `max`, `min`, and `lazy`; all other query parameters are
    /// retained for the selected SQLx transport. Invalid values and a minimum
    /// larger than the maximum return a structured configuration error.
    pub fn from_url(url: &str) -> Result<Self, DbError> {
        let mut parsed = url::Url::parse(url).map_err(|_| configuration_error("invalid URL"))?;

        let mut max_connections = 10;
        let mut min_connections = 1;
        let mut lazy = false;
        let mut transport_options = Vec::new();
        for (key, value) in parsed.query_pairs() {
            match key.as_ref() {
                "max" => max_connections = parse_pool_count("max", &value)?,
                "min" => min_connections = parse_pool_count("min", &value)?,
                "lazy" => lazy = parse_lazy(&value)?,
                _ => transport_options.push((key.into_owned(), value.into_owned())),
            }
        }
        if min_connections > max_connections {
            return Err(configuration_error("min cannot exceed max"));
        }

        parsed
            .query_pairs_mut()
            .clear()
            .extend_pairs(transport_options);

        Ok(Self {
            url: parsed.to_string(),
            min_connections,
            max_connections,
            lazy,
        })
    }
}

fn parse_pool_count(name: &str, value: &str) -> Result<u32, DbError> {
    let count = value
        .parse::<u32>()
        .map_err(|_| configuration_error(&format!("{name} must be an unsigned integer")))?;
    if name == "max" && count == 0 {
        return Err(configuration_error("max must be greater than zero"));
    }
    Ok(count)
}

fn parse_lazy(value: &str) -> Result<bool, DbError> {
    value
        .parse::<bool>()
        .map_err(|_| configuration_error("lazy must be true or false"))
}

fn configuration_error(reason: &str) -> DbError {
    DbError::from_sqlx(
        DbOperation::Connect,
        sqlx::Error::InvalidArgument(reason.to_string()),
    )
}

/// Observes a SQLx operation without exposing SQL text or bound values.
async fn observe_query<T, F>(operation: DbOperation, sql: &str, future: F) -> Result<T, sqlx::Error>
where
    F: Future<Output = Result<T, sqlx::Error>>,
{
    #[cfg(feature = "tracing")]
    let started = Instant::now();
    let result = future.await;
    #[cfg(feature = "tracing")]
    tracing::debug!(
        operation = %operation,
        query_fingerprint = query_fingerprint(sql),
        elapsed_micros = started.elapsed().as_micros() as u64,
        success = result.is_ok(),
        "database operation completed"
    );
    #[cfg(not(feature = "tracing"))]
    let _ = (operation, sql);
    result
}

#[cfg(feature = "tracing")]
fn query_fingerprint(sql: &str) -> u64 {
    let mut hasher = twox_hash::XxHash64::with_seed(0);
    hasher.write(sql.as_bytes());
    hasher.finish()
}

#[derive(Debug, Clone)]
/// Owned SQLx pool implementing [`DbSession`].
pub struct DbPool {
    pool: Pool,
}

impl DbPool {
    /// Returns the underlying SQLx pool for advanced backend operations.
    #[inline]
    pub fn as_sqlx(&self) -> &Pool {
        &self.pool
    }

    /// Wraps an existing selected-backend SQLx pool.
    pub fn from_pool(pool: Pool) -> Self {
        Self { pool }
    }

    /// Creates a SQLx pool from validated Mool configuration.
    pub async fn from_conf(conf: &DbConf) -> Result<Self, DbError> {
        let builder = sqlx::pool::PoolOptions::<Database>::new()
            .min_connections(conf.min_connections)
            .max_connections(conf.max_connections);

        let pool = if conf.lazy {
            builder
                .connect_lazy(&conf.url)
                .map_err(|error| DbError::from_sqlx(DbOperation::Connect, error))?
        } else {
            builder
                .connect(&conf.url)
                .await
                .map_err(|error| DbError::from_sqlx(DbOperation::Connect, error))?
        };

        Ok(Self { pool })
    }

    /// Begins a transaction that rolls back on drop unless completed explicitly.
    pub async fn begin(&self) -> Result<DbTransaction<'_>, DbError> {
        let tx = self
            .pool
            .begin()
            .await
            .map_err(|error| DbError::from_sqlx(DbOperation::Begin, error))?;
        Ok(DbTransaction { transaction: tx })
    }

    /// Runs a callback in a transaction, committing only when it returns `Ok`.
    ///
    /// The callback must return [`TransactionFuture`], usually by wrapping an
    /// `async move` block in [`Box::pin`]. Callback errors trigger an explicit
    /// rollback before the original error is returned.
    pub async fn transaction<T, F>(&self, callback: F) -> Result<T, DbError>
    where
        T: Send,
        F: for<'tx> FnOnce(&'tx mut DbTransaction<'_>) -> TransactionFuture<'tx, T>,
    {
        let mut transaction = self.begin().await?;
        match callback(&mut transaction).await {
            Ok(value) => {
                transaction.commit().await?;
                Ok(value)
            }
            Err(error) => {
                if let Err(rollback) = transaction.rollback().await {
                    return Err(DbError::RollbackFailed {
                        original: Box::new(error),
                        rollback: Box::new(rollback),
                    });
                }
                Err(error)
            }
        }
    }
}

impl DbSession for DbPool {
    async fn execute(&mut self, qs: Statement) -> Result<u64, DbError> {
        let (sql, args) = qs.into_parts().map_err(DbError::from)?;
        let query = sqlx::query_with(&sql, args);
        let res = observe_query(DbOperation::Execute, &sql, query.execute(&self.pool))
            .await
            .map_err(|error| DbError::from_sqlx(DbOperation::Execute, error))?;
        Ok(res.rows_affected())
    }

    async fn fetch_scalar<T>(&mut self, qs: Statement) -> Result<T, DbError>
    where
        for<'d> T: sqlx::Decode<'d, Database> + sqlx::Type<Database> + Send + Unpin,
    {
        let (sql, args) = qs.into_parts().map_err(DbError::from)?;
        let query = sqlx::query_scalar_with(&sql, args);
        observe_query(DbOperation::FetchScalar, &sql, query.fetch_one(&self.pool))
            .await
            .map_err(|error| DbError::from_sqlx(DbOperation::FetchScalar, error))
    }

    async fn fetch_one<M>(&mut self, qs: Statement) -> Result<M, DbError>
    where
        M: for<'r> sqlx::FromRow<'r, Row> + Send + Unpin,
    {
        let (sql, args) = qs.into_parts().map_err(DbError::from)?;
        let query = sqlx::query_as_with(&sql, args);
        observe_query(DbOperation::FetchOne, &sql, query.fetch_one(&self.pool))
            .await
            .map_err(|error| DbError::from_sqlx(DbOperation::FetchOne, error))
    }

    async fn fetch_all<M>(&mut self, qs: Statement) -> Result<Vec<M>, DbError>
    where
        M: for<'r> sqlx::FromRow<'r, Row> + Send + Unpin,
    {
        let (sql, args) = qs.into_parts().map_err(DbError::from)?;
        let query = sqlx::query_as_with(&sql, args);
        observe_query(DbOperation::FetchAll, &sql, query.fetch_all(&self.pool))
            .await
            .map_err(|error| DbError::from_sqlx(DbOperation::FetchAll, error))
    }

    async fn fetch_optional<M>(&mut self, qs: Statement) -> Result<Option<M>, DbError>
    where
        M: for<'r> sqlx::FromRow<'r, Row> + Send + Unpin,
    {
        let (sql, args) = qs.into_parts().map_err(DbError::from)?;
        let query = sqlx::query_as_with(&sql, args);
        observe_query(
            DbOperation::FetchOptional,
            &sql,
            query.fetch_optional(&self.pool),
        )
        .await
        .map_err(|error| DbError::from_sqlx(DbOperation::FetchOptional, error))
    }
}

impl DbSession for DbTransaction<'_> {
    async fn execute(&mut self, qs: Statement) -> Result<u64, DbError> {
        let (sql, args) = qs.into_parts().map_err(DbError::from)?;
        let query = sqlx::query_with(&sql, args);
        let res = observe_query(
            DbOperation::Execute,
            &sql,
            query.execute(&mut *self.transaction),
        )
        .await
        .map_err(|error| DbError::from_sqlx(DbOperation::Execute, error))?;
        Ok(res.rows_affected())
    }

    async fn fetch_scalar<T>(&mut self, qs: Statement) -> Result<T, DbError>
    where
        for<'d> T: sqlx::Decode<'d, Database> + sqlx::Type<Database> + Send + Unpin,
    {
        let (sql, args) = qs.into_parts().map_err(DbError::from)?;
        let query = sqlx::query_scalar_with(&sql, args);
        observe_query(
            DbOperation::FetchScalar,
            &sql,
            query.fetch_one(&mut *self.transaction),
        )
        .await
        .map_err(|error| DbError::from_sqlx(DbOperation::FetchScalar, error))
    }

    async fn fetch_one<M>(&mut self, qs: Statement) -> Result<M, DbError>
    where
        M: for<'r> sqlx::FromRow<'r, Row> + Send + Unpin,
    {
        let (sql, args) = qs.into_parts().map_err(DbError::from)?;
        let query = sqlx::query_as_with(&sql, args);
        observe_query(
            DbOperation::FetchOne,
            &sql,
            query.fetch_one(&mut *self.transaction),
        )
        .await
        .map_err(|error| DbError::from_sqlx(DbOperation::FetchOne, error))
    }

    async fn fetch_all<M>(&mut self, qs: Statement) -> Result<Vec<M>, DbError>
    where
        M: for<'r> sqlx::FromRow<'r, Row> + Send + Unpin,
    {
        let (sql, args) = qs.into_parts().map_err(DbError::from)?;
        let query = sqlx::query_as_with(&sql, args);
        observe_query(
            DbOperation::FetchAll,
            &sql,
            query.fetch_all(&mut *self.transaction),
        )
        .await
        .map_err(|error| DbError::from_sqlx(DbOperation::FetchAll, error))
    }

    async fn fetch_optional<M>(&mut self, qs: Statement) -> Result<Option<M>, DbError>
    where
        M: for<'r> sqlx::FromRow<'r, Row> + Send + Unpin,
    {
        let (sql, args) = qs.into_parts().map_err(DbError::from)?;
        let query = sqlx::query_as_with(&sql, args);
        observe_query(
            DbOperation::FetchOptional,
            &sql,
            query.fetch_optional(&mut *self.transaction),
        )
        .await
        .map_err(|error| DbError::from_sqlx(DbOperation::FetchOptional, error))
    }
}
