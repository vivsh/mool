//! Common database type definitions for all backends.
//!
//! This module provides database and argument type aliases based on the active backend feature.
//! Exactly one database backend must be enabled at a time.

#[cfg(all(feature = "postgres", feature = "mysql"))]
compile_error!(
    "database backend features are mutually exclusive: disable either `postgres` or `mysql`"
);

#[cfg(all(feature = "postgres", feature = "sqlite"))]
compile_error!(
    "database backend features are mutually exclusive: disable either `postgres` or `sqlite`"
);

#[cfg(all(feature = "mysql", feature = "sqlite"))]
compile_error!(
    "database backend features are mutually exclusive: disable either `mysql` or `sqlite`"
);

#[cfg(all(feature = "mariadb", feature = "postgres"))]
compile_error!(
    "database backend features are mutually exclusive: disable either `mariadb` or `postgres`"
);

#[cfg(all(feature = "mariadb", feature = "mysql"))]
compile_error!(
    "database backend features are mutually exclusive: disable either `mariadb` or `mysql`"
);

#[cfg(all(feature = "mariadb", feature = "sqlite"))]
compile_error!(
    "database backend features are mutually exclusive: disable either `mariadb` or `sqlite`"
);

#[cfg(not(any(
    feature = "postgres",
    feature = "mysql",
    feature = "mariadb",
    feature = "sqlite"
)))]
compile_error!(
    "enable exactly one database backend feature: `postgres`, `sqlite`, `mysql`, or `mariadb`"
);

// PostgreSQL types (default)
#[cfg(feature = "postgres")]
pub type Database = sqlx::Postgres;
#[cfg(feature = "postgres")]
pub type Arguments<'q> = sqlx::postgres::PgArguments;
#[cfg(feature = "postgres")]
pub type Row = sqlx::postgres::PgRow;
#[cfg(feature = "postgres")]
pub type QueryResult = sqlx::postgres::PgQueryResult;
#[cfg(feature = "postgres")]
pub type Pool = sqlx::PgPool;

// MySQL types
#[cfg(any(feature = "mysql", feature = "mariadb"))]
pub type Database = sqlx::MySql;
#[cfg(any(feature = "mysql", feature = "mariadb"))]
pub type Arguments<'q> = sqlx::mysql::MySqlArguments;
#[cfg(any(feature = "mysql", feature = "mariadb"))]
pub type Row = sqlx::mysql::MySqlRow;
#[cfg(any(feature = "mysql", feature = "mariadb"))]
pub type Pool = sqlx::MySqlPool;
#[cfg(any(feature = "mysql", feature = "mariadb"))]
pub type QueryResult = sqlx::mysql::MySqlQueryResult;

// SQLite types
#[cfg(feature = "sqlite")]
pub type Database = sqlx::Sqlite;
#[cfg(feature = "sqlite")]
pub type Arguments<'q> = sqlx::sqlite::SqliteArguments<'q>;
#[cfg(feature = "sqlite")]
pub type Row = sqlx::sqlite::SqliteRow;
#[cfg(feature = "sqlite")]
pub type Pool = sqlx::SqlitePool;
#[cfg(feature = "sqlite")]
pub type QueryResult = sqlx::sqlite::SqliteQueryResult;
