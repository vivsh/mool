//! PostgreSQL-native datetime and interval expressions.

use std::marker::PhantomData;

use sqlx::postgres::types::PgInterval;

use crate::QueryError;
use crate::queries::funcs::custom;
use crate::queries::{DbExpression, Expr, ExprRenderCtx, FunctionArgs, IntoExpr, val};

use super::traits::{NonNullSqlTimestamp, SqlNaiveTimestamp, SqlTimestamp};

/// Adds a native PostgreSQL interval to a timestamp expression.
pub fn add_interval<T>(timestamp: impl IntoExpr<T>, interval: PgInterval) -> Expr<T>
where
    T: SqlTimestamp,
{
    custom(PgBinaryExpr::<T>::new(
        PgBinaryOp::AddInterval,
        timestamp.into_expr(),
        val(interval),
    ))
}

/// Subtracts a native PostgreSQL interval from a timestamp expression.
pub fn subtract_interval<T>(timestamp: impl IntoExpr<T>, interval: PgInterval) -> Expr<T>
where
    T: SqlTimestamp,
{
    custom(PgBinaryExpr::<T>::new(
        PgBinaryOp::SubtractInterval,
        timestamp.into_expr(),
        val(interval),
    ))
}

/// Returns PostgreSQL's symbolic interval between two non-null timestamps.
pub fn age<T>(end: impl IntoExpr<T>, start: impl IntoExpr<T>) -> Expr<PgInterval>
where
    T: NonNullSqlTimestamp,
{
    custom(PgBinaryExpr::<PgInterval>::new(
        PgBinaryOp::Age,
        end.into_expr(),
        start.into_expr(),
    ))
}

/// Bins a timestamp into a PostgreSQL interval aligned to `origin`.
pub fn date_bin<T>(
    stride: PgInterval,
    source: impl IntoExpr<T>,
    origin: impl IntoExpr<T>,
) -> Expr<T>
where
    T: NonNullSqlTimestamp,
{
    custom(DateBinExpr::<T>::new(
        val(stride),
        source.into_expr(),
        origin.into_expr(),
    ))
}

/// Converts a UTC timestamp to a naive wall-clock timestamp in `zone`.
pub fn at_time_zone<T>(timestamp: impl IntoExpr<T>, zone: impl Into<String>) -> Expr<T::Naive>
where
    T: SqlTimestamp,
{
    custom(TimeZoneExpr::<T::Naive>::new(
        timestamp.into_expr(),
        zone.into(),
    ))
}

/// Assigns `zone` to a naive timestamp and returns the corresponding UTC instant.
pub fn assume_time_zone<T>(
    timestamp: impl IntoExpr<T>,
    zone: impl Into<String>,
) -> Expr<T::Timestamp>
where
    T: SqlNaiveTimestamp,
{
    custom(TimeZoneExpr::<T::Timestamp>::new(
        timestamp.into_expr(),
        zone.into(),
    ))
}

/// Formats a timestamp using PostgreSQL's `to_char` format language.
pub fn to_char<T>(timestamp: impl IntoExpr<T>, format: impl Into<String>) -> Expr<String>
where
    T: NonNullSqlTimestamp,
{
    custom(PgFormatExpr::new(timestamp.into_expr(), format.into()))
}

/// Returns PostgreSQL's transaction start timestamp.
pub fn transaction_timestamp<T>() -> Expr<T>
where
    T: NonNullSqlTimestamp,
{
    custom(PgCurrentExpr::<T>::new("TRANSACTION_TIMESTAMP()"))
}

/// Returns the wall-clock timestamp at the instant the expression is evaluated.
pub fn clock_timestamp<T>() -> Expr<T>
where
    T: NonNullSqlTimestamp,
{
    custom(PgCurrentExpr::<T>::new("CLOCK_TIMESTAMP()"))
}

#[derive(Clone, Copy)]
enum PgBinaryOp {
    AddInterval,
    SubtractInterval,
    Age,
}

struct PgBinaryExpr<R> {
    op: PgBinaryOp,
    args: FunctionArgs,
    _marker: PhantomData<fn() -> R>,
}

impl<R> Clone for PgBinaryExpr<R> {
    fn clone(&self) -> Self {
        Self {
            op: self.op,
            args: self.args.clone(),
            _marker: PhantomData,
        }
    }
}

impl<R> PgBinaryExpr<R> {
    fn new<L, T>(op: PgBinaryOp, left: Expr<L>, right: Expr<T>) -> Self {
        Self {
            op,
            args: FunctionArgs::new((left, right)),
            _marker: PhantomData,
        }
    }
}

impl<R: 'static> DbExpression<R> for PgBinaryExpr<R> {
    fn args(&self) -> FunctionArgs {
        self.args.clone()
    }

    fn render(&self, ctx: &mut ExprRenderCtx<'_>) -> Result<(), QueryError> {
        let (prefix, separator, suffix) = match self.op {
            PgBinaryOp::AddInterval => ("(", " + ", ")"),
            PgBinaryOp::SubtractInterval => ("(", " - ", ")"),
            PgBinaryOp::Age => ("AGE(", ", ", ")"),
        };
        ctx.push_sql(prefix);
        ctx.push_arg(0)?;
        ctx.push_sql(separator);
        ctx.push_arg(1)?;
        ctx.push_sql(suffix);
        Ok(())
    }
}

struct DateBinExpr<T> {
    args: FunctionArgs,
    _marker: PhantomData<fn() -> T>,
}

impl<T> Clone for DateBinExpr<T> {
    fn clone(&self) -> Self {
        Self {
            args: self.args.clone(),
            _marker: PhantomData,
        }
    }
}

impl<T> DateBinExpr<T> {
    fn new(stride: Expr<PgInterval>, source: Expr<T>, origin: Expr<T>) -> Self {
        Self {
            args: FunctionArgs::new((stride, source, origin)),
            _marker: PhantomData,
        }
    }
}

impl<T: 'static> DbExpression<T> for DateBinExpr<T> {
    fn args(&self) -> FunctionArgs {
        self.args.clone()
    }

    fn render(&self, ctx: &mut ExprRenderCtx<'_>) -> Result<(), QueryError> {
        ctx.push_sql("DATE_BIN(");
        ctx.push_arg(0)?;
        ctx.push_sql(", ");
        ctx.push_arg(1)?;
        ctx.push_sql(", ");
        ctx.push_arg(2)?;
        ctx.push_sql(")");
        Ok(())
    }
}

struct TimeZoneExpr<R> {
    args: FunctionArgs,
    _marker: PhantomData<fn() -> R>,
}

impl<R> Clone for TimeZoneExpr<R> {
    fn clone(&self) -> Self {
        Self {
            args: self.args.clone(),
            _marker: PhantomData,
        }
    }
}

impl<R> TimeZoneExpr<R> {
    fn new<T>(timestamp: Expr<T>, zone: String) -> Self {
        Self {
            args: FunctionArgs::new((timestamp, val(zone))),
            _marker: PhantomData,
        }
    }
}

impl<R: 'static> DbExpression<R> for TimeZoneExpr<R> {
    fn args(&self) -> FunctionArgs {
        self.args.clone()
    }

    fn render(&self, ctx: &mut ExprRenderCtx<'_>) -> Result<(), QueryError> {
        ctx.push_sql("(");
        ctx.push_arg(0)?;
        ctx.push_sql(" AT TIME ZONE ");
        ctx.push_arg(1)?;
        ctx.push_sql(")");
        Ok(())
    }
}

#[derive(Clone)]
struct PgFormatExpr {
    args: FunctionArgs,
}

impl PgFormatExpr {
    fn new<T>(timestamp: Expr<T>, format: String) -> Self {
        Self {
            args: FunctionArgs::new((timestamp, val(format))),
        }
    }
}

impl DbExpression<String> for PgFormatExpr {
    fn args(&self) -> FunctionArgs {
        self.args.clone()
    }

    fn render(&self, ctx: &mut ExprRenderCtx<'_>) -> Result<(), QueryError> {
        ctx.push_sql("TO_CHAR(");
        ctx.push_arg(0)?;
        ctx.push_sql(", ");
        ctx.push_arg(1)?;
        ctx.push_sql(")");
        Ok(())
    }
}

struct PgCurrentExpr<T> {
    sql: &'static str,
    _marker: PhantomData<fn() -> T>,
}

impl<T> Clone for PgCurrentExpr<T> {
    fn clone(&self) -> Self {
        Self {
            sql: self.sql,
            _marker: PhantomData,
        }
    }
}

impl<T> PgCurrentExpr<T> {
    fn new(sql: &'static str) -> Self {
        Self {
            sql,
            _marker: PhantomData,
        }
    }
}

impl<T: 'static> DbExpression<T> for PgCurrentExpr<T> {
    fn render(&self, ctx: &mut ExprRenderCtx<'_>) -> Result<(), QueryError> {
        ctx.push_sql(self.sql);
        Ok(())
    }
}
