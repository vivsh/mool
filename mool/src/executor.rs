#![allow(async_fn_in_trait)]

use crate::{Database, Pool, QueryError, Row, Statement};
use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Debug)]
pub enum IntegrityKind {
    Unique,
    ForeignKey,
    Check,
    NotNull,
    Exclusion,
    Other(String),
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
    #[error("integrity violation: {kind}")]
    Integrity {
        kind: IntegrityKind,
        constraint: Option<String>,
        #[source]
        source: sqlx::Error,
    },
    #[error("query returned multiple rows")]
    MultipleObjects,
    #[error("record not found")]
    DoesNotExist,
    #[error("temporary database failure")]
    Temporary,
    #[error("QuerySet error: {0}")]
    QuerySet(#[from] QueryError),
    #[error("unhandled db error")]
    Fatal(sqlx::Error),
    #[error("bad query")]
    BadQuery,
    #[error("feature not supported: {0}")]
    Unsupported(&'static str),
}

impl DbError {
    pub const fn code(&self) -> &'static str {
        match self {
            DbError::Integrity { .. } => "integrity_violation",
            DbError::MultipleObjects => "multiple_objects",
            DbError::DoesNotExist => "not_found",
            DbError::Temporary => "temporary_error",
            DbError::QuerySet(_) => "statement_error",
            DbError::Fatal(_) => "fatal_error",
            DbError::BadQuery => "bad_query",
            DbError::Unsupported(_) => "unsupported_feature",
        }
    }
}

impl From<sqlx::Error> for DbError {
    fn from(e: sqlx::Error) -> Self {
        match &e {
            sqlx::Error::RowNotFound => DbError::DoesNotExist,
            sqlx::Error::Database(db) => {
                #[cfg(feature = "postgres")]
                let kind = match db.code().as_deref() {
                    Some("23505") => IntegrityKind::Unique,
                    Some("23503") => IntegrityKind::ForeignKey,
                    Some("23514") => IntegrityKind::Check,
                    Some("23502") => IntegrityKind::NotNull,
                    Some("23P01") => IntegrityKind::Exclusion,
                    c => IntegrityKind::Other(c.unwrap_or_default().into()),
                };

                #[cfg(feature = "mysql")]
                let kind = match db.code().as_deref() {
                    Some("1062") => IntegrityKind::Unique,
                    Some("1451") | Some("1452") => IntegrityKind::ForeignKey,
                    Some("3819") => IntegrityKind::Check,
                    Some("1048") => IntegrityKind::NotNull,
                    c => IntegrityKind::Other(c.unwrap_or_default().into()),
                };

                #[cfg(feature = "sqlite")]
                let kind = match db.code().as_deref() {
                    Some("1555") | Some("2067") => IntegrityKind::Unique,
                    Some("787") => IntegrityKind::ForeignKey,
                    Some("275") => IntegrityKind::Check,
                    Some("1299") => IntegrityKind::NotNull,
                    c => IntegrityKind::Other(c.unwrap_or_default().into()),
                };

                #[cfg(not(any(feature = "postgres", feature = "mysql", feature = "sqlite")))]
                let kind = IntegrityKind::Other(db.code().unwrap_or_default().to_string());

                DbError::Integrity {
                    kind,
                    constraint: db.constraint().map(|s| s.to_owned()),
                    source: e,
                }
            }
            sqlx::Error::Io(_) | sqlx::Error::Tls(_) => DbError::Temporary,
            _ => DbError::Fatal(e),
        }
    }
}

pub trait DBSession {
    async fn execute(&mut self, qs: Statement) -> Result<u64, DbError>;

    async fn fetch_one<M>(&mut self, qs: Statement) -> Result<M, DbError>
    where
        M: for<'r> sqlx::FromRow<'r, Row> + Send + Unpin + 'static;

    async fn fetch_all<M>(&mut self, qs: Statement) -> Result<Vec<M>, DbError>
    where
        M: for<'r> sqlx::FromRow<'r, Row> + Send + Unpin + 'static;

    async fn fetch_optional<M>(&mut self, qs: Statement) -> Result<Option<M>, DbError>
    where
        M: for<'r> sqlx::FromRow<'r, Row> + Send + Unpin + 'static;

    async fn fetch_scalar<T>(&mut self, qs: Statement) -> Result<T, DbError>
    where
        for<'d> T: sqlx::Decode<'d, Database> + sqlx::Type<Database> + Send + Unpin + 'static;
}

pub struct DbTransaction<'a> {
    transaction: sqlx::Transaction<'a, Database>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DbConf {
    pub url: String,
    pub min_connections: u32,
    pub max_connections: u32,
    pub lazy: bool,
}

impl Default for DbConf {
    /// Default configuration is always valid and zero-cost until first use.
    /// Uses feature-dependent URLs: sqlite::memory, postgres://localhost/test, or mysql://localhost/test
    fn default() -> Self {
        #[cfg(feature = "sqlite")]
        let url = "sqlite::memory:";

        #[cfg(all(feature = "postgres", not(feature = "sqlite")))]
        let url = "postgres://localhost/test";

        #[cfg(all(feature = "mysql", not(any(feature = "postgres", feature = "sqlite"))))]
        let url = "mysql://localhost/test";

        // Dummy mode: shared in-memory SQLite — all pool connections see the same
        // database. Works without any external server.
        #[cfg(not(any(feature = "postgres", feature = "mysql", feature = "sqlite")))]
        let url = "sqlite:file::memory:?cache=shared&mode=memory";

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
            DbError::Fatal(sqlx::Error::Configuration("DATABASE_URL not set".into()))
        })?;

        Self::from_url(&url)
    }

    /// Parse configuration from a database URL string.
    /// Supports query parameters: max, min, lazy
    pub fn from_url(url: &str) -> Result<Self, DbError> {
        let parsed = url::Url::parse(url).map_err(|_| {
            DbError::Fatal(sqlx::Error::Configuration("invalid database URL".into()))
        })?;

        let mut max_connections = 10;
        let mut min_connections = 1;
        let mut lazy = false;
        for (key, value) in parsed.query_pairs() {
            match key.as_ref() {
                "max" => max_connections = value.parse().unwrap_or(max_connections),
                "min" => min_connections = value.parse().unwrap_or(min_connections),
                "lazy" => lazy = value.parse().unwrap_or(lazy),
                _ => {}
            }
        }

        // Remove query params from URL for the connection string
        let clean_url = Self::strip_query_params(url);

        Ok(Self {
            url: clean_url,
            min_connections,
            max_connections,
            lazy,
        })
    }

    fn strip_query_params(url: &str) -> String {
        url.split('?').next().unwrap_or(url).to_string()
    }
}

#[derive(Debug, Clone)]
pub struct DbPool {
    pool: Pool,
}

impl DbPool {
    #[inline]
    pub fn as_sqlx(&self) -> &Pool {
        &self.pool
    }

    pub fn from_pool(pool: Pool) -> Self {
        Self { pool }
    }

    pub async fn from_conf(conf: &DbConf) -> Result<Self, DbError> {
        let builder = sqlx::pool::PoolOptions::<Database>::new()
            .min_connections(conf.min_connections)
            .max_connections(conf.max_connections);

        let pool = if conf.lazy {
            builder.connect_lazy(&conf.url).map_err(DbError::Fatal)?
        } else {
            builder.connect(&conf.url).await.map_err(DbError::Fatal)?
        };

        Ok(Self { pool })
    }

    pub async fn begin(&self) -> Result<DbTransaction<'_>, DbError> {
        let tx = self.pool.begin().await?;
        Ok(DbTransaction { transaction: tx })
    }
}

impl DBSession for DbPool {
    async fn execute(&mut self, qs: Statement) -> Result<u64, DbError> {
        let (sql, args) = qs.into_parts().map_err(DbError::from)?;
        let query = sqlx::query_with(&sql, args);
        let res = query.execute(&self.pool).await?;
        Ok(res.rows_affected())
    }

    async fn fetch_scalar<T>(&mut self, qs: Statement) -> Result<T, DbError>
    where
        for<'d> T: sqlx::Decode<'d, Database> + sqlx::Type<Database> + Send + Unpin,
    {
        let (sql, args) = qs.into_parts().map_err(DbError::from)?;
        let query = sqlx::query_scalar_with(&sql, args);
        Ok(query.fetch_one(&self.pool).await?)
    }

    async fn fetch_one<M>(&mut self, qs: Statement) -> Result<M, DbError>
    where
        M: for<'r> sqlx::FromRow<'r, Row> + Send + Unpin,
    {
        let (sql, args) = qs.into_parts().map_err(DbError::from)?;
        let query = sqlx::query_as_with(&sql, args);
        Ok(query.fetch_one(&self.pool).await?)
    }

    async fn fetch_all<M>(&mut self, qs: Statement) -> Result<Vec<M>, DbError>
    where
        M: for<'r> sqlx::FromRow<'r, Row> + Send + Unpin,
    {
        let (sql, args) = qs.into_parts().map_err(DbError::from)?;
        let query = sqlx::query_as_with(&sql, args);
        Ok(query.fetch_all(&self.pool).await?)
    }

    async fn fetch_optional<M>(&mut self, qs: Statement) -> Result<Option<M>, DbError>
    where
        M: for<'r> sqlx::FromRow<'r, Row> + Send + Unpin,
    {
        let (sql, args) = qs.into_parts().map_err(DbError::from)?;
        let query = sqlx::query_as_with(&sql, args);
        Ok(query.fetch_optional(&self.pool).await?)
    }
}

impl DBSession for DbTransaction<'_> {
    async fn execute(&mut self, qs: Statement) -> Result<u64, DbError> {
        let (sql, args) = qs.into_parts().map_err(DbError::from)?;
        let query = sqlx::query_with(&sql, args);
        let res = query.execute(&mut *self.transaction).await?;
        Ok(res.rows_affected())
    }

    async fn fetch_scalar<T>(&mut self, qs: Statement) -> Result<T, DbError>
    where
        for<'d> T: sqlx::Decode<'d, Database> + sqlx::Type<Database> + Send + Unpin,
    {
        let (sql, args) = qs.into_parts().map_err(DbError::from)?;
        let query = sqlx::query_scalar_with(&sql, args);
        Ok(query.fetch_one(&mut *self.transaction).await?)
    }

    async fn fetch_one<M>(&mut self, qs: Statement) -> Result<M, DbError>
    where
        M: for<'r> sqlx::FromRow<'r, Row> + Send + Unpin,
    {
        let (sql, args) = qs.into_parts().map_err(DbError::from)?;
        let query = sqlx::query_as_with(&sql, args);
        Ok(query.fetch_one(&mut *self.transaction).await?)
    }

    async fn fetch_all<M>(&mut self, qs: Statement) -> Result<Vec<M>, DbError>
    where
        M: for<'r> sqlx::FromRow<'r, Row> + Send + Unpin,
    {
        let (sql, args) = qs.into_parts().map_err(DbError::from)?;
        let query = sqlx::query_as_with(&sql, args);
        Ok(query.fetch_all(&mut *self.transaction).await?)
    }

    async fn fetch_optional<M>(&mut self, qs: Statement) -> Result<Option<M>, DbError>
    where
        M: for<'r> sqlx::FromRow<'r, Row> + Send + Unpin,
    {
        let (sql, args) = qs.into_parts().map_err(DbError::from)?;
        let query = sqlx::query_as_with(&sql, args);
        Ok(query.fetch_optional(&mut *self.transaction).await?)
    }
}
