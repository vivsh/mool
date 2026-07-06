//! Portable aggregate SQL functions.

use super::super::expr::{Expr, IntoExpr};
use super::super::extension::func;
use super::common::WindowFn;

/// Creates a typed COUNT(...) expression.
pub fn count<T>(expr: impl IntoExpr<T>) -> Expr<i64> {
    func(WindowFn::new("COUNT", true, 1, 1), (expr.into_expr(),))
}

/// Creates a typed COUNT(*) expression.
pub fn count_all() -> Expr<i64> {
    func(WindowFn::new("COUNT", true, 0, 0), ())
}

/// Creates a typed SUM(...) expression.
pub fn sum<T>(expr: impl IntoExpr<T>) -> Expr<T>
where
    T: 'static,
{
    func(WindowFn::new("SUM", true, 1, 1), (expr.into_expr(),))
}

/// Creates a typed AVG(...) expression.
pub fn avg<T>(expr: impl IntoExpr<T>) -> Expr<f64>
where
    T: 'static,
{
    func(WindowFn::new("AVG", true, 1, 1), (expr.into_expr(),))
}

/// Creates a typed MIN(...) expression.
pub fn min<T>(expr: impl IntoExpr<T>) -> Expr<T>
where
    T: 'static,
{
    func(WindowFn::new("MIN", true, 1, 1), (expr.into_expr(),))
}

/// Creates a typed MAX(...) expression.
pub fn max<T>(expr: impl IntoExpr<T>) -> Expr<T>
where
    T: 'static,
{
    func(WindowFn::new("MAX", true, 1, 1), (expr.into_expr(),))
}
