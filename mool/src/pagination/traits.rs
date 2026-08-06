//! Traits for typed request-side pagination inputs.

use super::PaginationBuilder;

/// A request-side pagination input for a paginated query terminal.
///
/// Implementations configure page number and page size through
/// [`PaginationBuilder`]. They must not execute queries or mutate query
/// clauses.
pub trait Pageable {
    /// Configures pagination input through the provided builder.
    fn apply_page(&self, page: PaginationBuilder) -> PaginationBuilder;
}
