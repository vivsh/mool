//! Dialect-aware random ordering expression.

use crate::queries::{DbExpression, ExprRenderCtx, OrderExpr, funcs};
use crate::{QueryError, SqlDialect};

#[derive(Clone)]
struct RandomOrder;

impl DbExpression<i64> for RandomOrder {
    fn render(&self, ctx: &mut ExprRenderCtx<'_>) -> Result<(), QueryError> {
        let function = match ctx.dialect() {
            SqlDialect::Postgres | SqlDialect::Sqlite => "RANDOM()",
            SqlDialect::Mysql | SqlDialect::Mariadb => "RAND()",
        };
        ctx.push_sql(function);
        Ok(())
    }
}

/// Returns a dialect-aware random ORDER BY expression.
///
/// PostgreSQL and SQLite render `RANDOM()`; MySQL and MariaDB render `RAND()`.
/// The expression composes in the order supplied and may be expensive on large
/// result sets.
pub fn random_order() -> OrderExpr {
    funcs::custom(RandomOrder).asc()
}
