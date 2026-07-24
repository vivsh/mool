//! SQLite calendar and low-level datetime expressions.

use std::marker::PhantomData;

use crate::QueryError;
use crate::queries::funcs::custom;
use crate::queries::{DbExpression, Expr, ExprRenderCtx, FunctionArgs, IntoExpr, val};

use super::portable::{
    SqliteFamily, push_sqlite_format_end, push_sqlite_format_start, sqlite_family,
};
use super::traits::NonNullSqlTimestamp;
use super::traits::SqlTimestamp;

macro_rules! calendar_fn {
    ($name:ident, $unit:ident, $subtract:expr, $floor:expr, $doc:literal) => {
        #[doc = $doc]
        pub fn $name<T>(timestamp: impl IntoExpr<T>, amount: impl IntoExpr<i64>) -> Expr<T>
        where
            T: SqlTimestamp,
        {
            custom(SqliteCalendarExpr::<T>::new(
                CalendarUnit::$unit,
                $subtract,
                $floor,
                timestamp.into_expr(),
                amount.into_expr(),
            ))
        }
    };
}

calendar_fn!(
    add_calendar_months_floor,
    Month,
    false,
    true,
    "Adds calendar months and resolves ambiguous dates to the prior month's end."
);
calendar_fn!(
    add_calendar_months_ceiling,
    Month,
    false,
    false,
    "Adds calendar months using SQLite's later-date ambiguity rule."
);
calendar_fn!(
    subtract_calendar_months_floor,
    Month,
    true,
    true,
    "Subtracts calendar months and resolves ambiguous dates to the prior month's end."
);
calendar_fn!(
    subtract_calendar_months_ceiling,
    Month,
    true,
    false,
    "Subtracts calendar months using SQLite's later-date ambiguity rule."
);
calendar_fn!(
    add_calendar_years_floor,
    Year,
    false,
    true,
    "Adds calendar years and resolves leap-date ambiguity downward."
);
calendar_fn!(
    add_calendar_years_ceiling,
    Year,
    false,
    false,
    "Adds calendar years using SQLite's later-date ambiguity rule."
);
calendar_fn!(
    subtract_calendar_years_floor,
    Year,
    true,
    true,
    "Subtracts calendar years and resolves leap-date ambiguity downward."
);
calendar_fn!(
    subtract_calendar_years_ceiling,
    Year,
    true,
    false,
    "Subtracts calendar years using SQLite's later-date ambiguity rule."
);

/// Formats a non-null timestamp with a bound SQLite `strftime` format.
pub fn strftime<T>(timestamp: impl IntoExpr<T>, format: impl Into<String>) -> Expr<String>
where
    T: NonNullSqlTimestamp,
{
    custom(SqliteScalarExpr::new(
        ScalarOp::Strftime,
        timestamp.into_expr(),
        Some(format.into()),
    ))
}

/// Converts a non-null timestamp to a Julian day number.
pub fn julianday<T>(timestamp: impl IntoExpr<T>) -> Expr<f64>
where
    T: NonNullSqlTimestamp,
{
    custom(SqliteScalarExpr::new(
        ScalarOp::JulianDay,
        timestamp.into_expr(),
        None,
    ))
}

/// Converts a non-null timestamp to whole Unix epoch seconds.
pub fn unixepoch<T>(timestamp: impl IntoExpr<T>) -> Expr<i64>
where
    T: NonNullSqlTimestamp,
{
    custom(SqliteScalarExpr::new(
        ScalarOp::UnixEpoch,
        timestamp.into_expr(),
        None,
    ))
}

#[derive(Clone, Copy)]
enum CalendarUnit {
    Month,
    Year,
}

impl CalendarUnit {
    fn sql(self) -> &'static str {
        match self {
            Self::Month => "months",
            Self::Year => "years",
        }
    }
}

struct SqliteCalendarExpr<T> {
    unit: CalendarUnit,
    subtract: bool,
    floor: bool,
    family: SqliteFamily,
    args: FunctionArgs,
    _marker: PhantomData<fn() -> T>,
}

impl<T> Clone for SqliteCalendarExpr<T> {
    fn clone(&self) -> Self {
        Self {
            unit: self.unit,
            subtract: self.subtract,
            floor: self.floor,
            family: self.family,
            args: self.args.clone(),
            _marker: PhantomData,
        }
    }
}

impl<T> SqliteCalendarExpr<T> {
    fn new(
        unit: CalendarUnit,
        subtract: bool,
        floor: bool,
        timestamp: Expr<T>,
        amount: Expr<i64>,
    ) -> Self {
        Self {
            unit,
            subtract,
            floor,
            family: sqlite_family::<T>(),
            args: FunctionArgs::new((timestamp, amount)),
            _marker: PhantomData,
        }
    }
}

impl<T: 'static> DbExpression<T> for SqliteCalendarExpr<T> {
    fn args(&self) -> FunctionArgs {
        self.args.clone()
    }

    fn render(&self, ctx: &mut ExprRenderCtx<'_>) -> Result<(), QueryError> {
        push_sqlite_format_start(ctx, self.family, true);
        ctx.push_sql("datetime(");
        ctx.push_arg(0)?;
        ctx.push_sql(", printf('%+d ");
        ctx.push_sql(self.unit.sql());
        ctx.push_sql("', ");
        if self.subtract {
            ctx.push_sql("-(");
        }
        ctx.push_arg(1)?;
        if self.subtract {
            ctx.push_sql(")");
        }
        ctx.push_sql("), '");
        ctx.push_sql(if self.floor { "floor" } else { "ceiling" });
        ctx.push_sql("')");
        push_sqlite_format_end(ctx, self.family);
        Ok(())
    }
}

#[derive(Clone, Copy)]
enum ScalarOp {
    Strftime,
    JulianDay,
    UnixEpoch,
}

struct SqliteScalarExpr<R> {
    op: ScalarOp,
    args: FunctionArgs,
    _marker: PhantomData<fn() -> R>,
}

impl<R> Clone for SqliteScalarExpr<R> {
    fn clone(&self) -> Self {
        Self {
            op: self.op,
            args: self.args.clone(),
            _marker: PhantomData,
        }
    }
}

impl<R> SqliteScalarExpr<R> {
    fn new<T>(op: ScalarOp, timestamp: Expr<T>, format: Option<String>) -> Self {
        let args = match format {
            Some(format) => FunctionArgs::new((timestamp, val(format))),
            None => FunctionArgs::new((timestamp,)),
        };
        Self {
            op,
            args,
            _marker: PhantomData,
        }
    }
}

impl<R: 'static> DbExpression<R> for SqliteScalarExpr<R> {
    fn args(&self) -> FunctionArgs {
        self.args.clone()
    }

    fn render(&self, ctx: &mut ExprRenderCtx<'_>) -> Result<(), QueryError> {
        match self.op {
            ScalarOp::Strftime => {
                ctx.push_sql("strftime(");
                ctx.push_arg(1)?;
                ctx.push_sql(", ");
                ctx.push_arg(0)?;
                ctx.push_sql(")");
            }
            ScalarOp::JulianDay => push_scalar_arg(ctx, "julianday(")?,
            ScalarOp::UnixEpoch => push_scalar_arg(ctx, "unixepoch(")?,
        }
        Ok(())
    }
}

/// Renders a single-argument SQLite scalar call without exposing raw fragments.
fn push_scalar_arg(ctx: &mut ExprRenderCtx<'_>, prefix: &str) -> Result<(), QueryError> {
    ctx.push_sql(prefix);
    ctx.push_arg(0)?;
    ctx.push_sql(")");
    Ok(())
}
