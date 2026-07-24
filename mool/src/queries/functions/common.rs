//! Portable typed SQL functions and expressions.

use std::borrow::Cow;

use super::super::expr::{Expr, IntoExpr, Predicate};
use super::super::extension::{
    DbExpression, DbFunction, ExprRenderCtx, FunctionArgs, custom, func,
};
use crate::QueryError;

/// Returns the database statement timestamp as a Chrono UTC expression.
pub fn now() -> Expr<chrono::DateTime<chrono::Utc>> {
    crate::datetime::portable::now()
}

/// Creates a typed COALESCE(expr, fallback) expression.
pub fn coalesce<T>(expr: impl IntoExpr<T>, fallback: impl IntoExpr<T>) -> Expr<T>
where
    T: 'static,
{
    func(
        WindowFn::new("COALESCE", false, 2, 2),
        (expr.into_expr(), fallback.into_expr()),
    )
}

/// Starts a typed SQL CASE expression.
pub fn case() -> CaseStart {
    CaseStart
}

/// Creates a typed ROW_NUMBER() window function.
pub fn row_number() -> Expr<i64> {
    func(WindowFn::new("ROW_NUMBER", true, 0, 0), ())
}

/// Creates a typed RANK() window function.
pub fn rank() -> Expr<i64> {
    func(WindowFn::new("RANK", true, 0, 0), ())
}

/// Creates a typed DENSE_RANK() window function.
pub fn dense_rank() -> Expr<i64> {
    func(WindowFn::new("DENSE_RANK", true, 0, 0), ())
}

/// Creates a typed PERCENT_RANK() window function.
pub fn percent_rank() -> Expr<f64> {
    func(WindowFn::new("PERCENT_RANK", true, 0, 0), ())
}

/// Creates a typed CUME_DIST() window function.
pub fn cume_dist() -> Expr<f64> {
    func(WindowFn::new("CUME_DIST", true, 0, 0), ())
}

/// Creates a typed NTILE(n) window function.
pub fn ntile(n: impl IntoExpr<i64>) -> Expr<i64> {
    func(WindowFn::new("NTILE", true, 1, 1), (n.into_expr(),))
}

/// Creates a typed LAG(expr) window function.
pub fn lag<T>(expr: impl IntoExpr<T>) -> Expr<T>
where
    T: 'static,
{
    func(WindowFn::new("LAG", true, 1, 3), (expr.into_expr(),))
}

/// Creates a typed LAG(expr, offset) window function.
pub fn lag_by<T>(expr: impl IntoExpr<T>, offset: impl IntoExpr<i64>) -> Expr<T>
where
    T: 'static,
{
    func(
        WindowFn::new("LAG", true, 1, 3),
        (expr.into_expr(), offset.into_expr()),
    )
}

/// Creates a typed LAG(expr, offset, default) window function.
pub fn lag_or<T>(
    expr: impl IntoExpr<T>,
    offset: impl IntoExpr<i64>,
    default: impl IntoExpr<T>,
) -> Expr<T>
where
    T: 'static,
{
    func(
        WindowFn::new("LAG", true, 1, 3),
        (expr.into_expr(), offset.into_expr(), default.into_expr()),
    )
}

/// Creates a typed LEAD(expr) window function.
pub fn lead<T>(expr: impl IntoExpr<T>) -> Expr<T>
where
    T: 'static,
{
    func(WindowFn::new("LEAD", true, 1, 3), (expr.into_expr(),))
}

/// Creates a typed LEAD(expr, offset) window function.
pub fn lead_by<T>(expr: impl IntoExpr<T>, offset: impl IntoExpr<i64>) -> Expr<T>
where
    T: 'static,
{
    func(
        WindowFn::new("LEAD", true, 1, 3),
        (expr.into_expr(), offset.into_expr()),
    )
}

/// Creates a typed LEAD(expr, offset, default) window function.
pub fn lead_or<T>(
    expr: impl IntoExpr<T>,
    offset: impl IntoExpr<i64>,
    default: impl IntoExpr<T>,
) -> Expr<T>
where
    T: 'static,
{
    func(
        WindowFn::new("LEAD", true, 1, 3),
        (expr.into_expr(), offset.into_expr(), default.into_expr()),
    )
}

/// Creates a typed FIRST_VALUE(expr) window function.
pub fn first_value<T>(expr: impl IntoExpr<T>) -> Expr<T>
where
    T: 'static,
{
    func(
        WindowFn::new("FIRST_VALUE", true, 1, 1),
        (expr.into_expr(),),
    )
}

/// Creates a typed LAST_VALUE(expr) window function.
pub fn last_value<T>(expr: impl IntoExpr<T>) -> Expr<T>
where
    T: 'static,
{
    func(WindowFn::new("LAST_VALUE", true, 1, 1), (expr.into_expr(),))
}

/// Creates a typed NTH_VALUE(expr, n) window function.
pub fn nth_value<T>(expr: impl IntoExpr<T>, n: impl IntoExpr<i64>) -> Expr<T>
where
    T: 'static,
{
    func(
        WindowFn::new("NTH_VALUE", true, 2, 2),
        (expr.into_expr(), n.into_expr()),
    )
}

#[derive(Clone)]
pub(super) struct WindowFn {
    name: &'static str,
    window: bool,
    min_arity: usize,
    max_arity: usize,
}

impl WindowFn {
    pub(super) fn new(
        name: &'static str,
        window: bool,
        min_arity: usize,
        max_arity: usize,
    ) -> Self {
        Self {
            name,
            window,
            min_arity,
            max_arity,
        }
    }
}

impl<T> DbFunction<T> for WindowFn {
    fn name(&self, _dialect: crate::SqlDialect) -> Result<Cow<'static, str>, QueryError> {
        Ok(Cow::Borrowed(self.name))
    }

    fn validate(&self, _dialect: crate::SqlDialect, arity: usize) -> Result<(), QueryError> {
        if (self.min_arity..=self.max_arity).contains(&arity) {
            return Ok(());
        }
        Err(QueryError::BindError(format!(
            "{} requires {} argument(s)",
            self.name,
            arity_label(self.min_arity, self.max_arity)
        )))
    }

    fn supports_window(&self) -> bool {
        self.window
    }
}

fn arity_label(min: usize, max: usize) -> String {
    if min == max {
        return min.to_string();
    }
    format!("{min}-{max}")
}

/// Untyped start state for a SQL CASE expression.
pub struct CaseStart;

/// Typed CASE expression builder.
pub struct CaseBuilder<T> {
    args: FunctionArgs,
    arms: usize,
    _marker: std::marker::PhantomData<fn() -> T>,
}

impl CaseStart {
    /// Adds the first WHEN/THEN arm and fixes the CASE result type.
    pub fn when<T>(self, predicate: Predicate, value: impl IntoExpr<T>) -> CaseBuilder<T>
    where
        T: 'static,
    {
        CaseBuilder::new().when(predicate, value)
    }
}

impl<T> CaseBuilder<T>
where
    T: 'static,
{
    fn new() -> Self {
        Self {
            args: FunctionArgs::default(),
            arms: 0,
            _marker: std::marker::PhantomData,
        }
    }

    /// Adds another WHEN/THEN arm with the same result type.
    pub fn when(mut self, predicate: Predicate, value: impl IntoExpr<T>) -> Self {
        self.args.nodes.push(predicate.into_node());
        self.args.nodes.push(value.into_expr().node);
        self.arms += 1;
        self
    }

    /// Completes the CASE expression with an ELSE value.
    pub fn else_(mut self, fallback: impl IntoExpr<T>) -> Expr<T> {
        self.args.nodes.push(fallback.into_expr().node);
        custom(CaseExpression {
            args: self.args,
            arms: self.arms,
            _marker: std::marker::PhantomData,
        })
    }
}

struct CaseExpression<T> {
    args: FunctionArgs,
    arms: usize,
    _marker: std::marker::PhantomData<fn() -> T>,
}

impl<T> Clone for CaseExpression<T> {
    fn clone(&self) -> Self {
        Self {
            args: self.args.clone(),
            arms: self.arms,
            _marker: std::marker::PhantomData,
        }
    }
}

impl<T> DbExpression<T> for CaseExpression<T>
where
    T: 'static,
{
    fn args(&self) -> FunctionArgs {
        self.args.clone()
    }

    fn render(&self, ctx: &mut ExprRenderCtx<'_>) -> Result<(), QueryError> {
        ctx.push_sql("CASE");
        for arm in 0..self.arms {
            ctx.push_sql(" WHEN ");
            ctx.push_arg(arm * 2)?;
            ctx.push_sql(" THEN ");
            ctx.push_arg(arm * 2 + 1)?;
        }
        ctx.push_sql(" ELSE ");
        ctx.push_arg(self.arms * 2)?;
        ctx.push_sql(" END");
        Ok(())
    }
}
