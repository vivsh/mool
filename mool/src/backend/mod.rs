//! Types and capabilities for the selected database backend.

#[cfg(not(mool_has_backend))]
pub use crate::backendless::{Arguments, Database, Pool, QueryResult, Row};
#[cfg(mool_has_backend)]
pub use crate::commons::{Arguments, Database, Pool, QueryResult, Row};

#[cfg(any(feature = "postgres", feature = "mysql", feature = "mariadb"))]
mod locking;
#[cfg(feature = "mariadb")]
mod mariadb;
#[cfg(feature = "mysql")]
mod mysql;
#[cfg(feature = "postgres")]
mod postgres;
#[cfg(feature = "sqlite")]
mod sqlite;
#[cfg(feature = "postgres")]
mod unnest;

#[cfg(any(feature = "postgres", feature = "mysql"))]
pub use locking::LockWaitExt;
#[cfg(any(feature = "postgres", feature = "mysql", feature = "mariadb"))]
pub use locking::RowLockExt;
#[cfg(feature = "mariadb")]
pub use mariadb::*;
#[cfg(feature = "mysql")]
pub use mysql::*;
#[cfg(feature = "postgres")]
pub use postgres::*;
#[cfg(feature = "sqlite")]
pub use sqlite::*;
#[cfg(feature = "postgres")]
pub use unnest::{PgBatchColumns, PostgresUnnestExt};

#[cfg(any(feature = "postgres", feature = "sqlite"))]
use crate::queries::InsertConflict;
#[cfg(any(feature = "postgres", feature = "sqlite"))]
use crate::queries::{BatchInsert, ColumnSet, ReturningBatchInsert};

/// Exact duplicate-conflict handling for batch inserts.
#[cfg(any(feature = "postgres", feature = "sqlite"))]
pub trait IgnoreConflictsExt: Sized {
    /// Ignores rows conflicting with any applicable unique constraint.
    fn ignore_conflicts(self) -> Self;

    /// Ignores rows conflicting with the selected unique columns.
    fn ignore_conflicts_on<C>(self, columns: C) -> Self
    where
        C: ColumnSet;
}

#[cfg(any(feature = "postgres", feature = "sqlite"))]
impl<T> IgnoreConflictsExt for BatchInsert<'_, T> {
    fn ignore_conflicts(mut self) -> Self {
        self.conflict = InsertConflict::Ignore(None);
        self
    }

    fn ignore_conflicts_on<C>(mut self, columns: C) -> Self
    where
        C: ColumnSet,
    {
        self.conflict = InsertConflict::Ignore(Some(columns.into_column_refs()));
        self
    }
}

#[cfg(any(feature = "postgres", feature = "sqlite"))]
impl<R, T> IgnoreConflictsExt for ReturningBatchInsert<'_, R, T> {
    fn ignore_conflicts(mut self) -> Self {
        self.conflict = InsertConflict::Ignore(None);
        self
    }

    fn ignore_conflicts_on<C>(mut self, columns: C) -> Self
    where
        C: ColumnSet,
    {
        self.conflict = InsertConflict::Ignore(Some(columns.into_column_refs()));
        self
    }
}

#[cfg(any(feature = "postgres", feature = "sqlite"))]
impl<T> IgnoreConflictsExt for crate::queries::OwnedBatchInsert<T> {
    fn ignore_conflicts(mut self) -> Self {
        self.conflict = InsertConflict::Ignore(None);
        self
    }

    fn ignore_conflicts_on<C>(mut self, columns: C) -> Self
    where
        C: ColumnSet,
    {
        self.conflict = InsertConflict::Ignore(Some(columns.into_column_refs()));
        self
    }
}

#[cfg(feature = "postgres")]
impl<T> IgnoreConflictsExt for crate::queries::PgUnnestBatchInsert<'_, T> {
    fn ignore_conflicts(mut self) -> Self {
        self.inner.conflict = InsertConflict::Ignore(None);
        self
    }

    fn ignore_conflicts_on<C>(mut self, columns: C) -> Self
    where
        C: ColumnSet,
    {
        self.inner.conflict = InsertConflict::Ignore(Some(columns.into_column_refs()));
        self
    }
}

#[cfg(feature = "postgres")]
impl<R, T> IgnoreConflictsExt for crate::queries::ReturningPgUnnestBatchInsert<'_, R, T> {
    fn ignore_conflicts(mut self) -> Self {
        self.inner.conflict = InsertConflict::Ignore(None);
        self
    }

    fn ignore_conflicts_on<C>(mut self, columns: C) -> Self
    where
        C: ColumnSet,
    {
        self.inner.conflict = InsertConflict::Ignore(Some(columns.into_column_refs()));
        self
    }
}

#[cfg(feature = "postgres")]
impl<T> IgnoreConflictsExt for crate::queries::OwnedPgUnnestBatchInsert<T> {
    fn ignore_conflicts(mut self) -> Self {
        self.inner.conflict = InsertConflict::Ignore(None);
        self
    }

    fn ignore_conflicts_on<C>(mut self, columns: C) -> Self
    where
        C: ColumnSet,
    {
        self.inner.conflict = InsertConflict::Ignore(Some(columns.into_column_refs()));
        self
    }
}

/// MySQL-family broad `INSERT IGNORE` behavior.
#[cfg(any(feature = "mysql", feature = "mariadb"))]
pub trait IgnoreErrorsExt: Sized {
    /// Converts insert errors, including duplicate keys, into server warnings.
    fn ignore_errors(self) -> Self;
}

#[cfg(any(feature = "mysql", feature = "mariadb"))]
impl<T> IgnoreErrorsExt for crate::queries::BatchInsert<'_, T> {
    fn ignore_errors(mut self) -> Self {
        self.conflict = crate::queries::InsertConflict::IgnoreErrors;
        self
    }
}

#[cfg(any(feature = "mysql", feature = "mariadb"))]
impl<T> IgnoreErrorsExt for crate::queries::OwnedBatchInsert<T> {
    fn ignore_errors(mut self) -> Self {
        self.conflict = crate::queries::InsertConflict::IgnoreErrors;
        self
    }
}

/// Returns the maximum rows per batch for a fixed number of bound columns.
#[cfg(mool_has_backend)]
pub fn max_batch_rows(column_count: usize) -> Option<usize> {
    let rows = PARAMETER_LIMIT.checked_div(column_count)?;
    (rows > 0).then_some(rows)
}

#[cfg(mool_has_backend)]
pub(crate) const fn gaman_dialect() -> gaman::core::Dialect {
    #[cfg(feature = "postgres")]
    return gaman::core::Dialect::Postgres;
    #[cfg(feature = "sqlite")]
    return gaman::core::Dialect::Sqlite;
    #[cfg(feature = "mysql")]
    return gaman::core::Dialect::Mysql;
    #[cfg(feature = "mariadb")]
    return gaman::core::Dialect::Mariadb;
}
