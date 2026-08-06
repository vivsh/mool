//! Pagination builder used by `Pageable` implementations.

use crate::QueryError;

use super::Pagination;

/// Request-side pagination builder passed to `Pageable` implementations.
///
/// It accumulates page components so validation is deferred until a paginated
/// query terminal resolves the builder.
#[derive(Default)]
pub struct PaginationBuilder {
    page_num: Option<usize>,
    page_size: Option<usize>,
}

impl PaginationBuilder {
    /// Starts an empty pagination input.
    pub fn new() -> Self {
        Self::default()
    }

    /// Sets the requested one-indexed page number.
    pub fn page_num(mut self, page_num: usize) -> Self {
        self.page_num = Some(page_num);
        self
    }

    /// Sets the requested number of rows per page.
    pub fn page_size(mut self, page_size: usize) -> Self {
        self.page_size = Some(page_size);
        self
    }

    pub(crate) fn build(self) -> Result<Pagination, QueryError> {
        let page_num = self.page_num.ok_or(QueryError::MissingPageNumber)?;
        let page_size = self.page_size.ok_or(QueryError::MissingPageSize)?;
        Ok(Pagination {
            page_num,
            page_size,
        })
    }
}
