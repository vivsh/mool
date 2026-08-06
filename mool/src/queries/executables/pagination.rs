//! Paginated read helper built from count and slice executables.

use crate::commons::Arguments;
use crate::commons::Row;
use crate::executor::{DbError, DbSession};
use crate::interfaces::Record;
use crate::pagination::{Page, Pageable, Pagination, PaginationBuilder};
use crate::placeholders::Dialect;

use super::super::binds::statement_from_plan;
use super::super::scope::QueryScope;
use crate::QueryError;

impl QueryScope {
    /// Executes a paginated read by running count and slice queries.
    ///
    /// `Pagination` is one-indexed and normalizes zero values to one.
    pub async fn page<T, S>(
        self,
        pagination: Pagination,
        session: &mut S,
    ) -> Result<Page<T>, DbError>
    where
        T: Record + for<'r> sqlx::FromRow<'r, Row> + Send + Unpin + 'static,
        S: DbSession,
    {
        let page = pagination.page_num.max(1);
        let per_page = pagination.page_size.max(1);
        let offset = (page - 1)
            .checked_mul(per_page)
            .ok_or(QueryError::PaginationOverflow { page, per_page })?;
        let count_stmt =
            statement_from_plan(self.plan_count(Dialect::active())?, Arguments::default())?;
        let total = session.fetch_scalar(count_stmt).await?;
        let items = self.slice::<T>(offset, per_page).exec(session).await?;
        Ok(Page::new(items, total, page, per_page))
    }

    /// Resolves a typed pagination input and executes a paginated read.
    pub async fn page_with<T, P, S>(self, pageable: &P, session: &mut S) -> Result<Page<T>, DbError>
    where
        T: Record + for<'r> sqlx::FromRow<'r, Row> + Send + Unpin + 'static,
        P: Pageable,
        S: DbSession,
    {
        let pagination = pageable.apply_page(PaginationBuilder::new()).build()?;
        self.page(pagination, session).await
    }
}
