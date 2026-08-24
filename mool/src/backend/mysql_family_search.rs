//! MySQL-family full-text query modes.

use crate::queries::__private::HasCols;
use crate::queries::{IntoExpr, ModelTable, Predicate, TextQueryMode};

/// MySQL-family `MATCH ... AGAINST` query mode.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MySqlFullTextMode {
    /// Natural-language relevance search.
    NaturalLanguage,
    /// Boolean full-text operator syntax.
    Boolean,
}

impl From<MySqlFullTextMode> for TextQueryMode {
    fn from(mode: MySqlFullTextMode) -> Self {
        match mode {
            MySqlFullTextMode::NaturalLanguage => Self::NaturalLanguage,
            MySqlFullTextMode::Boolean => Self::Boolean,
        }
    }
}

/// MySQL-family full-text modes beyond the portable natural-language API.
pub trait MySqlFullTextExt {
    /// Matches a named full-text definition using the selected mode.
    fn text_search_mode<Q>(&self, name: &str, query: Q, mode: MySqlFullTextMode) -> Predicate
    where
        Q: IntoExpr<String>;

    /// Computes relevance using the selected mode.
    fn text_relevance_mode<Q>(
        &self,
        name: &str,
        query: Q,
        mode: MySqlFullTextMode,
    ) -> crate::queries::Expr<f64>
    where
        Q: IntoExpr<String>;
}

impl<M> MySqlFullTextExt for ModelTable<M>
where
    M: crate::Model + HasCols,
{
    fn text_search_mode<Q>(&self, name: &str, query: Q, mode: MySqlFullTextMode) -> Predicate
    where
        Q: IntoExpr<String>,
    {
        self.text_search_internal(name, query, mode.into())
    }

    fn text_relevance_mode<Q>(
        &self,
        name: &str,
        query: Q,
        mode: MySqlFullTextMode,
    ) -> crate::queries::Expr<f64>
    where
        Q: IntoExpr<String>,
    {
        self.text_relevance_internal(name, query, mode.into())
    }
}
