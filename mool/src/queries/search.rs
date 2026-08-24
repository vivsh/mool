//! Typed search expressions over model-bound table and column metadata.

#[cfg(feature = "text-search")]
use std::sync::Arc;

#[cfg(feature = "text-search")]
use crate::Model;

#[cfg(any(feature = "fuzzy-search", feature = "vector"))]
use super::expr::ColumnRef;
#[cfg(feature = "fuzzy-search")]
use super::expr::FuzzyOperation;
#[cfg(any(feature = "text-search", feature = "fuzzy-search"))]
use super::expr::Predicate;
#[cfg(feature = "vector")]
use super::expr::VectorOperation;
use super::expr::{Expr, ExprNode, IntoExpr};
#[cfg(feature = "text-search")]
use super::expr::{TextQueryMode, TextSearchResult};
#[cfg(any(feature = "fuzzy-search", feature = "vector"))]
use super::handles::Column;
#[cfg(feature = "text-search")]
use super::handles::ModelTable;
#[cfg(feature = "text-search")]
use super::traits::HasCols;

#[cfg(feature = "text-search")]
impl<M> ModelTable<M>
where
    M: Model + HasCols,
{
    /// Matches a natural-language query against a named table full-text definition.
    pub fn text_search<Q>(&self, name: &str, query: Q) -> Predicate
    where
        Q: IntoExpr<String>,
    {
        let mode = if cfg!(feature = "postgres") {
            TextQueryMode::Plain
        } else {
            TextQueryMode::NaturalLanguage
        };
        self.text_search_internal(name, query, mode)
    }

    /// Computes relevance for a named table full-text definition.
    pub fn text_relevance<Q>(&self, name: &str, query: Q) -> Expr<f64>
    where
        Q: IntoExpr<String>,
    {
        let mode = if cfg!(feature = "postgres") {
            TextQueryMode::Plain
        } else {
            TextQueryMode::NaturalLanguage
        };
        self.text_relevance_internal(name, query, mode)
    }

    pub(crate) fn text_search_internal<Q>(
        &self,
        name: &str,
        query: Q,
        mode: TextQueryMode,
    ) -> Predicate
    where
        Q: IntoExpr<String>,
    {
        Predicate::new(ExprNode::TextSearch {
            table: self.table_source(),
            name: Arc::from(name),
            query: Box::new(query.into_expr().node),
            result: TextSearchResult::Matches,
            mode,
        })
    }

    pub(crate) fn text_relevance_internal<Q>(
        &self,
        name: &str,
        query: Q,
        mode: TextQueryMode,
    ) -> Expr<f64>
    where
        Q: IntoExpr<String>,
    {
        Expr::new(ExprNode::TextSearch {
            table: self.table_source(),
            name: Arc::from(name),
            query: Box::new(query.into_expr().node),
            result: TextSearchResult::Relevance,
            mode,
        })
    }
}

/// Exact trigram similarity operations for declared fuzzy text columns.
#[cfg(feature = "fuzzy-search")]
pub trait FuzzyTextExt {
    /// Computes PostgreSQL trigram similarity in the range zero to one.
    fn fuzzy_similarity<Q>(&self, query: Q) -> Expr<f64>
    where
        Q: IntoExpr<String>;

    /// Computes trigram distance for nearest-neighbour ordering.
    fn fuzzy_distance<Q>(&self, query: Q) -> Expr<f64>
    where
        Q: IntoExpr<String>;

    /// Tests similarity against an explicit, query-local threshold.
    fn fuzzy_match<Q, T>(&self, query: Q, minimum_similarity: T) -> Predicate
    where
        Q: IntoExpr<String>,
        T: IntoExpr<f64>;
}

#[cfg(feature = "fuzzy-search")]
impl FuzzyTextExt for Column<String> {
    fn fuzzy_similarity<Q>(&self, query: Q) -> Expr<f64>
    where
        Q: IntoExpr<String>,
    {
        fuzzy_expr(self, query, FuzzyOperation::Similarity)
    }

    fn fuzzy_distance<Q>(&self, query: Q) -> Expr<f64>
    where
        Q: IntoExpr<String>,
    {
        fuzzy_expr(self, query, FuzzyOperation::Distance)
    }

    fn fuzzy_match<Q, T>(&self, query: Q, minimum_similarity: T) -> Predicate
    where
        Q: IntoExpr<String>,
        T: IntoExpr<f64>,
    {
        Predicate::new(ExprNode::Fuzzy {
            column: column_ref(self),
            query: Box::new(query.into_expr().node),
            operation: FuzzyOperation::MinimumSimilarity,
            threshold: Some(Box::new(minimum_similarity.into_expr().node)),
        })
    }
}

#[cfg(feature = "fuzzy-search")]
pub(crate) fn fuzzy_expr<Q>(
    column: &Column<String>,
    query: Q,
    operation: FuzzyOperation,
) -> Expr<f64>
where
    Q: IntoExpr<String>,
{
    Expr::new(ExprNode::Fuzzy {
        column: column_ref(column),
        query: Box::new(query.into_expr().node),
        operation,
        threshold: None,
    })
}

#[cfg(all(feature = "fuzzy-search", feature = "postgres"))]
pub(crate) fn fuzzy_predicate<Q>(
    column: &Column<String>,
    query: Q,
    operation: FuzzyOperation,
) -> Predicate
where
    Q: IntoExpr<String>,
{
    Predicate::new(ExprNode::Fuzzy {
        column: column_ref(column),
        query: Box::new(query.into_expr().node),
        operation,
        threshold: None,
    })
}

/// Portable distance and cosine-similarity operations for vector columns.
#[cfg(feature = "vector")]
pub trait VectorSimilarityExt {
    /// Computes Euclidean L2 distance.
    fn l2_distance<Q>(&self, query: Q) -> Expr<f64>
    where
        Q: IntoExpr<crate::types::Vector>;

    /// Computes cosine distance.
    fn cosine_distance<Q>(&self, query: Q) -> Expr<f64>
    where
        Q: IntoExpr<crate::types::Vector>;

    /// Computes cosine similarity as one minus cosine distance.
    fn cosine_similarity<Q>(&self, query: Q) -> Expr<f64>
    where
        Q: IntoExpr<crate::types::Vector>;
}

#[cfg(feature = "vector")]
impl VectorSimilarityExt for Column<crate::types::Vector> {
    fn l2_distance<Q>(&self, query: Q) -> Expr<f64>
    where
        Q: IntoExpr<crate::types::Vector>,
    {
        vector_expr(self, query, VectorOperation::L2Distance)
    }

    fn cosine_distance<Q>(&self, query: Q) -> Expr<f64>
    where
        Q: IntoExpr<crate::types::Vector>,
    {
        vector_expr(self, query, VectorOperation::CosineDistance)
    }

    fn cosine_similarity<Q>(&self, query: Q) -> Expr<f64>
    where
        Q: IntoExpr<crate::types::Vector>,
    {
        vector_expr(self, query, VectorOperation::CosineSimilarity)
    }
}

#[cfg(feature = "vector")]
pub(crate) fn vector_expr<Q>(
    column: &Column<crate::types::Vector>,
    query: Q,
    operation: VectorOperation,
) -> Expr<f64>
where
    Q: IntoExpr<crate::types::Vector>,
{
    Expr::new(ExprNode::VectorDistance {
        column: column_ref(column),
        query: Box::new(query.into_expr().node),
        operation,
    })
}

#[cfg(any(feature = "fuzzy-search", feature = "vector"))]
fn column_ref<T>(column: &Column<T>) -> ColumnRef {
    ColumnRef {
        owner: column.data.owner.clone(),
        name: column.data.name.clone(),
    }
}
