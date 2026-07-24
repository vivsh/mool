//! PostgreSQL SQL array expression helpers.

use std::marker::PhantomData;

use crate::ArgValue;
use crate::QueryError;
use crate::types::Array;

use super::super::expr::{Expr, ExprNode, IntoExpr, Predicate, ValueNode};
use super::super::extension::{DbExpression, ExprRenderCtx, FunctionArgs, custom};

/// Creates a typed SQL array literal bind value.
pub fn value<T>(value: Vec<T>) -> Expr<Array<T>>
where
    Vec<T>: Clone
        + for<'q> sqlx::Encode<'q, crate::backend::Database>
        + sqlx::Type<crate::backend::Database>
        + Send
        + Sync
        + 'static,
{
    Expr::new(ExprNode::Value(ValueNode::Val {
        name: None,
        rust_type: std::any::type_name::<Vec<T>>(),
        value: ArgValue::new(value),
    }))
}

/// Creates a `left @> right` containment predicate.
pub fn contains<T>(left: impl IntoExpr<Array<T>>, right: impl IntoExpr<Array<T>>) -> Predicate
where
    T: Send + Sync + 'static,
{
    custom::<bool, _>(ArrayBinaryExpr::<T>::new(
        ArrayBinaryOp::Contains,
        left.into_expr(),
        right.into_expr(),
    ))
    .into_predicate()
}

/// Creates a `left <@ right` contained-by predicate.
pub fn contained_by<T>(left: impl IntoExpr<Array<T>>, right: impl IntoExpr<Array<T>>) -> Predicate
where
    T: Send + Sync + 'static,
{
    custom::<bool, _>(ArrayBinaryExpr::<T>::new(
        ArrayBinaryOp::ContainedBy,
        left.into_expr(),
        right.into_expr(),
    ))
    .into_predicate()
}

/// Creates a `left && right` overlap predicate.
pub fn overlaps<T>(left: impl IntoExpr<Array<T>>, right: impl IntoExpr<Array<T>>) -> Predicate
where
    T: Send + Sync + 'static,
{
    custom::<bool, _>(ArrayBinaryExpr::<T>::new(
        ArrayBinaryOp::Overlaps,
        left.into_expr(),
        right.into_expr(),
    ))
    .into_predicate()
}

/// Checks whether an array is empty.
pub fn is_empty<T>(array: impl IntoExpr<Array<T>>) -> Predicate
where
    T: Send + Sync + 'static,
{
    custom::<bool, _>(ArrayUnaryExpr::<T>::new(
        ArrayUnaryOp::IsEmpty,
        array.into_expr(),
    ))
    .into_predicate()
}

/// Returns `array_length(array, 1)`.
pub fn length<T>(array: impl IntoExpr<Array<T>>) -> Expr<i64>
where
    T: Send + Sync + 'static,
{
    custom(ArrayUnaryExpr::<T>::new(
        ArrayUnaryOp::Length,
        array.into_expr(),
    ))
}

/// Returns `cardinality(array)`.
pub fn cardinality<T>(array: impl IntoExpr<Array<T>>) -> Expr<i64>
where
    T: Send + Sync + 'static,
{
    custom(ArrayUnaryExpr::<T>::new(
        ArrayUnaryOp::Cardinality,
        array.into_expr(),
    ))
}

/// Returns `array_position(array, value)`.
pub fn position<T>(array: impl IntoExpr<Array<T>>, value: impl IntoExpr<T>) -> Expr<i64>
where
    T: Send + Sync + 'static,
{
    custom(ArrayValueExpr::<T>::new(
        ArrayValueOp::Position,
        array.into_expr(),
        value.into_expr(),
    ))
}

/// Creates a `value = ANY(array)` predicate.
pub fn any<T>(array: impl IntoExpr<Array<T>>, value: impl IntoExpr<T>) -> Predicate
where
    T: Send + Sync + 'static,
{
    custom::<bool, _>(ArrayValueExpr::<T>::new(
        ArrayValueOp::Any,
        array.into_expr(),
        value.into_expr(),
    ))
    .into_predicate()
}

/// Creates a `value = ALL(array)` predicate.
pub fn all<T>(array: impl IntoExpr<Array<T>>, value: impl IntoExpr<T>) -> Predicate
where
    T: Send + Sync + 'static,
{
    custom::<bool, _>(ArrayValueExpr::<T>::new(
        ArrayValueOp::All,
        array.into_expr(),
        value.into_expr(),
    ))
    .into_predicate()
}

struct ArrayBinaryExpr<T> {
    op: ArrayBinaryOp,
    args: FunctionArgs,
    _marker: PhantomData<fn() -> T>,
}

impl<T> Clone for ArrayBinaryExpr<T> {
    fn clone(&self) -> Self {
        Self {
            op: self.op,
            args: self.args.clone(),
            _marker: PhantomData,
        }
    }
}

#[derive(Clone, Copy)]
enum ArrayBinaryOp {
    Contains,
    ContainedBy,
    Overlaps,
}

impl<T> ArrayBinaryExpr<T> {
    fn new(op: ArrayBinaryOp, left: Expr<Array<T>>, right: Expr<Array<T>>) -> Self {
        Self {
            op,
            args: FunctionArgs::new((left, right)),
            _marker: PhantomData,
        }
    }
}

impl<T> DbExpression<bool> for ArrayBinaryExpr<T>
where
    T: Send + Sync + 'static,
{
    fn args(&self) -> FunctionArgs {
        self.args.clone()
    }

    fn render(&self, ctx: &mut ExprRenderCtx<'_>) -> Result<(), QueryError> {
        ctx.push_sql("(");
        ctx.push_arg(0)?;
        ctx.push_sql(" ");
        ctx.push_sql(self.operator());
        ctx.push_sql(" ");
        ctx.push_arg(1)?;
        ctx.push_sql(")");
        Ok(())
    }
}

impl<T> ArrayBinaryExpr<T> {
    fn operator(&self) -> &'static str {
        match self.op {
            ArrayBinaryOp::Contains => "@>",
            ArrayBinaryOp::ContainedBy => "<@",
            ArrayBinaryOp::Overlaps => "&&",
        }
    }
}

struct ArrayUnaryExpr<T> {
    op: ArrayUnaryOp,
    args: FunctionArgs,
    _marker: PhantomData<fn() -> T>,
}

impl<T> Clone for ArrayUnaryExpr<T> {
    fn clone(&self) -> Self {
        Self {
            op: self.op,
            args: self.args.clone(),
            _marker: PhantomData,
        }
    }
}

#[derive(Clone, Copy)]
enum ArrayUnaryOp {
    IsEmpty,
    Length,
    Cardinality,
}

impl<T> ArrayUnaryExpr<T> {
    fn new(op: ArrayUnaryOp, array: Expr<Array<T>>) -> Self {
        Self {
            op,
            args: FunctionArgs::new((array,)),
            _marker: PhantomData,
        }
    }
}

impl<T> DbExpression<bool> for ArrayUnaryExpr<T>
where
    T: Send + Sync + 'static,
{
    fn args(&self) -> FunctionArgs {
        self.args.clone()
    }

    fn render(&self, ctx: &mut ExprRenderCtx<'_>) -> Result<(), QueryError> {
        ctx.push_sql("cardinality(");
        ctx.push_arg(0)?;
        ctx.push_sql(") = 0");
        Ok(())
    }
}

impl<T> DbExpression<i64> for ArrayUnaryExpr<T>
where
    T: Send + Sync + 'static,
{
    fn args(&self) -> FunctionArgs {
        self.args.clone()
    }

    fn render(&self, ctx: &mut ExprRenderCtx<'_>) -> Result<(), QueryError> {
        let suffix = match self.op {
            ArrayUnaryOp::Length => {
                ctx.push_sql("array_length(");
                ", 1)"
            }
            ArrayUnaryOp::Cardinality => {
                ctx.push_sql("cardinality(");
                ")"
            }
            ArrayUnaryOp::IsEmpty => Err(QueryError::BindError(
                "array empty check is a predicate".to_string(),
            ))?,
        };
        ctx.push_arg(0)?;
        ctx.push_sql(suffix);
        Ok(())
    }
}

struct ArrayValueExpr<T> {
    op: ArrayValueOp,
    args: FunctionArgs,
    _marker: PhantomData<fn() -> T>,
}

impl<T> Clone for ArrayValueExpr<T> {
    fn clone(&self) -> Self {
        Self {
            op: self.op,
            args: self.args.clone(),
            _marker: PhantomData,
        }
    }
}

#[derive(Clone, Copy)]
enum ArrayValueOp {
    Position,
    Any,
    All,
}

impl<T> ArrayValueExpr<T> {
    fn new(op: ArrayValueOp, array: Expr<Array<T>>, value: Expr<T>) -> Self {
        Self {
            op,
            args: FunctionArgs::new((array, value)),
            _marker: PhantomData,
        }
    }
}

impl<T> DbExpression<i64> for ArrayValueExpr<T>
where
    T: Send + Sync + 'static,
{
    fn args(&self) -> FunctionArgs {
        self.args.clone()
    }

    fn render(&self, ctx: &mut ExprRenderCtx<'_>) -> Result<(), QueryError> {
        ctx.push_sql("array_position(");
        ctx.push_arg(0)?;
        ctx.push_sql(", ");
        ctx.push_arg(1)?;
        ctx.push_sql(")");
        Ok(())
    }
}

impl<T> DbExpression<bool> for ArrayValueExpr<T>
where
    T: Send + Sync + 'static,
{
    fn args(&self) -> FunctionArgs {
        self.args.clone()
    }

    fn render(&self, ctx: &mut ExprRenderCtx<'_>) -> Result<(), QueryError> {
        let operator = match self.op {
            ArrayValueOp::Any => " = ANY(",
            ArrayValueOp::All => " = ALL(",
            ArrayValueOp::Position => Err(QueryError::BindError(
                "array position is a scalar expression".to_string(),
            ))?,
        };
        ctx.push_sql("(");
        ctx.push_arg(1)?;
        ctx.push_sql(operator);
        ctx.push_arg(0)?;
        ctx.push_sql("))");
        Ok(())
    }
}
