//! PostgreSQL-only typed query capabilities.

use crate::Record;
#[cfg(feature = "text-search")]
use crate::queries::__private::HasCols;
use crate::queries::__private::IntoTableSource;
#[cfg(feature = "fuzzy-search")]
use crate::queries::FuzzyOperation;
#[cfg(feature = "vector")]
use crate::queries::VectorOperation;
use crate::queries::{
    Column, Delete, IntoExpr, Predicate, ProjectedColumn, QueryScope, ReturningDelete,
    ReturningScope,
};
#[cfg(feature = "text-search")]
use crate::queries::{ModelTable, TextQueryMode};

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

/// PostgreSQL parser used to convert a text value into `tsquery` terms.
#[cfg(feature = "text-search")]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PostgresTextQueryMode {
    /// Treats the input as unstructured terms.
    Plain,
    /// Treats the input as one phrase.
    Phrase,
    /// Accepts PostgreSQL's web-search query syntax.
    WebSearch,
}

#[cfg(feature = "text-search")]
impl From<PostgresTextQueryMode> for TextQueryMode {
    fn from(mode: PostgresTextQueryMode) -> Self {
        match mode {
            PostgresTextQueryMode::Plain => Self::Plain,
            PostgresTextQueryMode::Phrase => Self::Phrase,
            PostgresTextQueryMode::WebSearch => Self::WebSearch,
        }
    }
}

/// PostgreSQL-specific full-text query parsing modes.
#[cfg(feature = "text-search")]
pub trait PostgresFullTextExt {
    /// Matches a named full-text definition using one PostgreSQL parser mode.
    fn text_search_mode<Q>(&self, name: &str, query: Q, mode: PostgresTextQueryMode) -> Predicate
    where
        Q: IntoExpr<String>;

    /// Computes relevance using one PostgreSQL parser mode.
    fn text_relevance_mode<Q>(
        &self,
        name: &str,
        query: Q,
        mode: PostgresTextQueryMode,
    ) -> crate::queries::Expr<f64>
    where
        Q: IntoExpr<String>;
}

#[cfg(feature = "text-search")]
impl<M> PostgresFullTextExt for ModelTable<M>
where
    M: crate::Model + HasCols,
{
    fn text_search_mode<Q>(&self, name: &str, query: Q, mode: PostgresTextQueryMode) -> Predicate
    where
        Q: IntoExpr<String>,
    {
        self.text_search_internal(name, query, mode.into())
    }

    fn text_relevance_mode<Q>(
        &self,
        name: &str,
        query: Q,
        mode: PostgresTextQueryMode,
    ) -> crate::queries::Expr<f64>
    where
        Q: IntoExpr<String>,
    {
        self.text_relevance_internal(name, query, mode.into())
    }
}

/// PostgreSQL-only trigram operators beyond the exact portable threshold API.
#[cfg(feature = "fuzzy-search")]
pub trait PostgresTrigramExt {
    /// Uses PostgreSQL's session-configured `%` similarity threshold.
    fn trigram_match<Q>(&self, query: Q) -> Predicate
    where
        Q: IntoExpr<String>;

    /// Computes the greatest trigram similarity against a contiguous word extent.
    fn word_similarity<Q>(&self, query: Q) -> crate::queries::Expr<f64>
    where
        Q: IntoExpr<String>;

    /// Computes strict word-boundary trigram similarity.
    fn strict_word_similarity<Q>(&self, query: Q) -> crate::queries::Expr<f64>
    where
        Q: IntoExpr<String>;
}

#[cfg(feature = "fuzzy-search")]
impl PostgresTrigramExt for Column<String> {
    fn trigram_match<Q>(&self, query: Q) -> Predicate
    where
        Q: IntoExpr<String>,
    {
        crate::queries::fuzzy_predicate(self, query, FuzzyOperation::ThresholdMatch)
    }

    fn word_similarity<Q>(&self, query: Q) -> crate::queries::Expr<f64>
    where
        Q: IntoExpr<String>,
    {
        crate::queries::fuzzy_expr(self, query, FuzzyOperation::WordSimilarity)
    }

    fn strict_word_similarity<Q>(&self, query: Q) -> crate::queries::Expr<f64>
    where
        Q: IntoExpr<String>,
    {
        crate::queries::fuzzy_expr(self, query, FuzzyOperation::StrictWordSimilarity)
    }
}

/// PostgreSQL pgvector operators not available in MariaDB's vector API.
#[cfg(feature = "vector")]
pub trait PostgresVectorExt {
    /// Computes pgvector's negative inner product, suitable for ascending index order.
    fn inner_product<Q>(&self, query: Q) -> crate::queries::Expr<f64>
    where
        Q: IntoExpr<crate::types::Vector>;

    /// Computes L1 taxicab distance.
    fn l1_distance<Q>(&self, query: Q) -> crate::queries::Expr<f64>
    where
        Q: IntoExpr<crate::types::Vector>;
}

#[cfg(feature = "vector")]
impl PostgresVectorExt for Column<crate::types::Vector> {
    fn inner_product<Q>(&self, query: Q) -> crate::queries::Expr<f64>
    where
        Q: IntoExpr<crate::types::Vector>,
    {
        crate::queries::vector_expr(self, query, VectorOperation::InnerProduct)
    }

    fn l1_distance<Q>(&self, query: Q) -> crate::queries::Expr<f64>
    where
        Q: IntoExpr<crate::types::Vector>,
    {
        crate::queries::vector_expr(self, query, VectorOperation::L1Distance)
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

/// PostgreSQL `DELETE ... USING` composition for typed mutation sources.
pub trait PostgresDeleteUsingExt: Sized {
    /// Attaches a typed table, CTE, or subquery used by this delete.
    fn using<S>(self, source: S) -> Self
    where
        S: IntoTableSource;
}

impl PostgresDeleteUsingExt for Delete {
    fn using<S>(self, source: S) -> Self
    where
        S: IntoTableSource,
    {
        self.with_mutation_source(source)
    }
}

impl<R> PostgresDeleteUsingExt for ReturningDelete<R>
where
    R: Record + 'static,
{
    fn using<S>(self, source: S) -> Self
    where
        S: IntoTableSource,
    {
        self.with_mutation_source(source)
    }
}

/// Name of the selected backend.
pub const NAME: &str = "postgres";

/// Maximum number of bind parameters accepted by one PostgreSQL statement.
pub const PARAMETER_LIMIT: usize = 65_535;
