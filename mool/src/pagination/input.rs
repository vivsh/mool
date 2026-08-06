//! Concrete pagination input for paginated query terminals.

/// One-indexed pagination input for a paginated query terminal.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Pagination {
    /// Requested one-indexed page number. Zero is normalized to one.
    pub page_num: usize,
    /// Requested number of rows per page. Zero is normalized to one.
    pub page_size: usize,
}
