/// A page of results from a paginated query.
#[derive(Clone, Debug, serde::Serialize, schemars::JsonSchema)]
pub struct Page<T> {
    pub items: Vec<T>,
    pub total: i64,
    pub page: usize,
    pub per_page: usize,
    pub total_pages: usize,
}

impl<T> Page<T> {
    /// Builds a page from a result slice, total row count, and one-indexed page.
    pub fn new(items: Vec<T>, total: i64, page: usize, per_page: usize) -> Self {
        let total_pages = if per_page == 0 {
            0
        } else {
            (total.max(0) as usize).div_ceil(per_page)
        };
        Self {
            items,
            total,
            page,
            per_page,
            total_pages,
        }
    }

    /// Maps page items while preserving pagination metadata.
    pub fn map<U>(self, f: impl FnMut(T) -> U) -> Page<U> {
        Page {
            items: self.items.into_iter().map(f).collect(),
            total: self.total,
            page: self.page,
            per_page: self.per_page,
            total_pages: self.total_pages,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::Page;

    /// Verifies that page metadata is derived from total rows and page size.
    #[test]
    fn page_new_sets_metadata() {
        let page = Page::new(vec![1, 2], 11, 2, 5);
        assert_eq!(page.page, 2);
        assert_eq!(page.per_page, 5);
        assert_eq!(page.total_pages, 3);
    }

    /// Verifies that mapping preserves pagination metadata.
    #[test]
    fn page_map_preserves_metadata() {
        let page = Page::new(vec![1, 2], 8, 1, 4).map(|item| item.to_string());
        assert_eq!(page.items, ["1", "2"]);
        assert_eq!(page.total, 8);
        assert_eq!(page.total_pages, 2);
    }
}
