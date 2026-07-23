//! Dialect-specific rendering and feature validation for typed queries.

use std::borrow::Cow;

use crate::placeholders::Dialect;

use super::expr::ColumnRef;
use super::validate::validate_identifier;
use crate::QueryError;

mod common;
mod features;
#[cfg(feature = "mariadb")]
mod mariadb;
#[cfg(feature = "mysql")]
mod mysql;
#[cfg(feature = "postgres")]
mod postgres;
#[cfg(feature = "sqlite")]
mod sqlite;

pub(super) use features::DialectFeature;

#[cfg(feature = "postgres")]
static POSTGRES: postgres::PostgresSpec = postgres::PostgresSpec;
#[cfg(feature = "sqlite")]
static SQLITE: sqlite::SqliteSpec = sqlite::SqliteSpec;
#[cfg(feature = "mysql")]
static MYSQL: mysql::MysqlSpec = mysql::MysqlSpec;
#[cfg(feature = "mariadb")]
static MARIADB: mariadb::MariadbSpec = mariadb::MariadbSpec;

/// Internal dialect renderer used by the typed-query planner.
///
/// Common SQL is rendered by the shared renderer. This trait owns only syntax
/// and feature points that genuinely differ between supported backends.
pub(super) trait DialectRenderer: Send + Sync {
    fn placeholder(&self, position: usize) -> String;

    fn validate_feature(&self, feature: DialectFeature) -> Result<(), QueryError>;

    fn render_upsert(
        &self,
        conflict: &[ColumnRef],
        update_columns: &[&str],
    ) -> Result<String, QueryError>;

    #[cfg(any(feature = "postgres", feature = "sqlite"))]
    fn render_ignore_conflicts(&self, conflict: &[ColumnRef]) -> Result<String, QueryError>;

    fn render_returning(&self, columns: &[String]) -> Result<String, QueryError> {
        self.validate_feature(DialectFeature::Returning)?;
        if columns.is_empty() {
            return Err(QueryError::BindError(
                "RETURNING requires at least one column".to_string(),
            ));
        }
        Ok(format!(" RETURNING {}", columns.join(", ")))
    }

    fn render_function(&self, name: Cow<'static, str>) -> Result<Cow<'static, str>, QueryError> {
        validate_identifier(&name)?;
        Ok(name)
    }
}

pub(super) fn renderer(dialect: Dialect) -> &'static dyn DialectRenderer {
    debug_assert_eq!(dialect, Dialect::active());
    active_renderer()
}

#[cfg(feature = "postgres")]
fn active_renderer() -> &'static dyn DialectRenderer {
    &POSTGRES
}

#[cfg(feature = "sqlite")]
fn active_renderer() -> &'static dyn DialectRenderer {
    &SQLITE
}

#[cfg(feature = "mysql")]
fn active_renderer() -> &'static dyn DialectRenderer {
    &MYSQL
}

#[cfg(feature = "mariadb")]
fn active_renderer() -> &'static dyn DialectRenderer {
    &MARIADB
}

pub(super) fn validate_feature(
    dialect: Dialect,
    feature: DialectFeature,
) -> Result<(), QueryError> {
    renderer(dialect).validate_feature(feature)
}

pub(super) fn render_function(
    dialect: Dialect,
    name: Cow<'static, str>,
) -> Result<Cow<'static, str>, QueryError> {
    renderer(dialect).render_function(name)
}
