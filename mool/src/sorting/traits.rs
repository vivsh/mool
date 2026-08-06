//! Traits for typed query ordering inputs.

use crate::interfaces::Model;
use crate::queries::__private::HasCols;

use super::SortBuilder;

/// A model-bound, ORDER BY-only typed sort input.
///
/// Implementations should append ordering expressions through
/// [`SortBuilder::sort`]. They must not execute queries or add predicates,
/// pagination, relation loading, writes, or other query behavior.
pub trait Sortable {
    /// The root model this ordering can be applied to.
    type Model: Model + HasCols;

    /// Appends this input's ordering expressions to the provided builder.
    fn apply_sort(&self, sort: SortBuilder<Self::Model>) -> SortBuilder<Self::Model>;
}
