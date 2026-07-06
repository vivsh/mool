//! Traits for typed query filter DTOs.

use crate::interfaces::Model;
use crate::queries::__private::HasCols;

use super::FilterBuilder;

/// A model-bound, WHERE-only typed filter.
///
/// Implementations should append predicates through [`FilterBuilder::filter`].
/// They must not execute queries or add ordering, pagination, relation loading,
/// writes, or any other non-WHERE behavior.
pub trait Filterable {
    /// The root model this filter can be applied to.
    type Model: Model + HasCols;

    /// Appends this filter's predicates to the provided builder.
    fn apply_filter(&self, filter: FilterBuilder<Self::Model>) -> FilterBuilder<Self::Model>;
}
