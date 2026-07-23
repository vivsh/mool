//! PostgreSQL-only typed query capabilities.

use crate::Record;
use crate::queries::{Column, IntoExpr, Predicate, ProjectedColumn, QueryScope, ReturningScope};

/// PostgreSQL case-insensitive text predicates.
pub trait TextSearchExt {
    /// Builds a PostgreSQL `ILIKE` predicate.
    fn ilike<R>(&self, rhs: R) -> Predicate
    where
        R: IntoExpr<String>;
}

impl TextSearchExt for Column<String> {
    fn ilike<R>(&self, rhs: R) -> Predicate
    where
        R: IntoExpr<String>,
    {
        self.compare_text("ILIKE", rhs)
    }
}

impl TextSearchExt for ProjectedColumn<String> {
    fn ilike<R>(&self, rhs: R) -> Predicate
    where
        R: IntoExpr<String>,
    {
        self.compare_text("ILIKE", rhs)
    }
}

/// PostgreSQL write projections using `RETURNING`.
pub trait ReturningExt {
    /// Selects the typed record returned by a write terminal.
    fn returning<R>(self) -> ReturningScope<R>
    where
        R: Record;
}

impl ReturningExt for QueryScope {
    fn returning<R>(self) -> ReturningScope<R>
    where
        R: Record,
    {
        self.into_returning()
    }
}

/// PostgreSQL `DISTINCT ON` composition for typed select scopes.
pub trait DistinctOnExt: Sized {
    /// Adds a typed expression to the PostgreSQL `DISTINCT ON` key.
    fn distinct_on<T>(self, expr: impl IntoExpr<T>) -> QueryScope;
}

impl DistinctOnExt for QueryScope {
    fn distinct_on<T>(self, expr: impl IntoExpr<T>) -> QueryScope {
        self.with_distinct_on(expr)
    }
}

/// Name of the selected backend.
pub const NAME: &str = "postgres";

/// Maximum number of bind parameters accepted by one PostgreSQL statement.
pub const PARAMETER_LIMIT: usize = 65_535;
