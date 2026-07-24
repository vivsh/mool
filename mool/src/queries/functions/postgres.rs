//! PostgreSQL-specific typed SQL helpers.

use std::borrow::Cow;

use super::super::expr::{Expr, IntoExpr};
use super::super::extension::{DbFunction, func};
use crate::QueryError;

/// PostgreSQL-specific datetime expressions.
pub mod datetime {
    pub use crate::datetime::postgres::*;
}

/// Creates a typed `unaccent(expr)` expression.
pub fn unaccent(expr: impl IntoExpr<String>) -> Expr<String> {
    func(Unaccent, (expr.into_expr(),))
}

#[derive(Clone, Copy, Debug)]
struct Unaccent;

impl DbFunction<String> for Unaccent {
    fn name(&self, _dialect: crate::SqlDialect) -> Result<Cow<'static, str>, QueryError> {
        Ok(Cow::Borrowed("unaccent"))
    }
}
