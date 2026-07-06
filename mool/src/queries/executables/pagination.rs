//! Paginated read helper built from count and slice executables.

use crate::Page;
use crate::commons::Arguments;
use crate::commons::Row;
use crate::executor::{DBSession, DbError};
use crate::interfaces::Record;
use crate::placeholders::Dialect;

use super::super::binds::statement_from_plan;
use super::super::scope::QueryScope;

impl QueryScope {
    /// Executes a paginated read by running count and slice queries.
    ///
    /// `page` is one-indexed and `per_page` is clamped to at least one.
    pub async fn page<T, S>(
        self,
        page: usize,
        per_page: usize,
        session: &mut S,
    ) -> Result<Page<T>, DbError>
    where
        T: Record + for<'r> sqlx::FromRow<'r, Row> + Send + Unpin + 'static,
        S: DBSession,
    {
        let page = page.max(1);
        let per_page = per_page.max(1);
        let count_stmt =
            statement_from_plan(self.plan_count(Dialect::active())?, Arguments::default())?;
        let total = session.fetch_scalar(count_stmt).await?;
        let items = self
            .slice::<T>((page - 1) * per_page, per_page)
            .exec(session)
            .await?;
        Ok(Page::new(items, total, page, per_page))
    }
}
