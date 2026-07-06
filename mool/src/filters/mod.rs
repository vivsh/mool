//! Typed WHERE-filter support for source-first database queries.
//!
//! Filters are model-bound predicate builders. Applying a filter only appends
//! predicates to the current query scope; it never executes another query.

mod builder;
mod traits;

pub use builder::FilterBuilder;
pub use traits::Filterable;

#[doc(hidden)]
pub mod __private {
    pub use super::builder::in_values;
}
