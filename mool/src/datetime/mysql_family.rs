//! MySQL-family calendar, timezone, and formatting expressions.

use std::marker::PhantomData;

use crate::QueryError;
use crate::queries::funcs::custom;
use crate::queries::{DbExpression, Expr, ExprRenderCtx, FunctionArgs, IntoExpr, val};

use super::traits::{NonNullSqlTimestamp, SqlTimestamp, SqlTimestampPair};

macro_rules! calendar_fn {
    ($name:ident, $unit:ident, $subtract:expr, $doc:literal) => {
        #[doc = $doc]
        pub fn $name<T>(timestamp: impl IntoExpr<T>, amount: impl IntoExpr<i64>) -> Expr<T>
        where
            T: SqlTimestamp,
        {
            custom(CalendarExpr::<T>::new(
                CalendarUnit::$unit,
                $subtract,
                timestamp.into_expr(),
                amount.into_expr(),
            ))
        }
    };
}

calendar_fn!(
    add_calendar_years,
    Year,
    false,
    "Adds calendar years using MySQL-family rollover rules."
);
calendar_fn!(
    add_calendar_months,
    Month,
    false,
    "Adds calendar months using MySQL-family rollover rules."
);
calendar_fn!(
    add_calendar_days,
    Day,
    false,
    "Adds calendar days using MySQL-family semantics."
);
calendar_fn!(
    subtract_calendar_years,
    Year,
    true,
    "Subtracts calendar years using MySQL-family rollover rules."
);
calendar_fn!(
    subtract_calendar_months,
    Month,
    true,
    "Subtracts calendar months using MySQL-family rollover rules."
);
calendar_fn!(
    subtract_calendar_days,
    Day,
    true,
    "Subtracts calendar days using MySQL-family semantics."
);

macro_rules! timestamp_diff_fn {
    ($name:ident, $unit:ident, $doc:literal) => {
        #[doc = $doc]
        pub fn $name<L, R>(
            start: impl IntoExpr<L>,
            end: impl IntoExpr<R>,
        ) -> Expr<<L as SqlTimestampPair<R>>::Difference>
        where
            L: SqlTimestamp + SqlTimestampPair<R>,
            R: SqlTimestamp,
        {
            custom(
                MySqlDiffExpr::<<L as SqlTimestampPair<R>>::Difference>::new(
                    CalendarUnit::$unit,
                    start.into_expr(),
                    end.into_expr(),
                ),
            )
        }
    };
}

timestamp_diff_fn!(
    timestamp_diff_years,
    Year,
    "Returns complete calendar years between two timestamps."
);
timestamp_diff_fn!(
    timestamp_diff_months,
    Month,
    "Returns complete calendar months between two timestamps."
);
timestamp_diff_fn!(
    timestamp_diff_days,
    Day,
    "Returns complete calendar days between two timestamps."
);
timestamp_diff_fn!(
    timestamp_diff_hours,
    Hour,
    "Returns complete hours using MySQL `TIMESTAMPDIFF` semantics."
);
timestamp_diff_fn!(
    timestamp_diff_minutes,
    Minute,
    "Returns complete minutes using MySQL `TIMESTAMPDIFF` semantics."
);
timestamp_diff_fn!(
    timestamp_diff_seconds,
    Second,
    "Returns complete seconds using MySQL `TIMESTAMPDIFF` semantics."
);

/// Converts a UTC timestamp into a naive wall-clock timestamp in `to_zone`.
///
/// Named zones require MySQL or MariaDB timezone tables. Zone names are bound
/// values and are never interpolated into SQL.
pub fn convert_time_zone<T>(
    timestamp: impl IntoExpr<T>,
    from_zone: impl Into<String>,
    to_zone: impl Into<String>,
) -> Expr<T::Naive>
where
    T: NonNullSqlTimestamp,
{
    custom(TimeZoneExpr::<T::Naive>::new(
        timestamp.into_expr(),
        from_zone.into(),
        to_zone.into(),
    ))
}

/// Formats a non-null timestamp using a MySQL-family date format string.
pub fn date_format<T>(timestamp: impl IntoExpr<T>, format: impl Into<String>) -> Expr<String>
where
    T: NonNullSqlTimestamp,
{
    custom(DateFormatExpr::new(timestamp.into_expr(), format.into()))
}

#[derive(Clone, Copy)]
enum CalendarUnit {
    Year,
    Month,
    Day,
    Hour,
    Minute,
    Second,
}

impl CalendarUnit {
    fn sql(self) -> &'static str {
        match self {
            Self::Year => "YEAR",
            Self::Month => "MONTH",
            Self::Day => "DAY",
            Self::Hour => "HOUR",
            Self::Minute => "MINUTE",
            Self::Second => "SECOND",
        }
    }
}

struct CalendarExpr<T> {
    unit: CalendarUnit,
    subtract: bool,
    args: FunctionArgs,
    _marker: PhantomData<fn() -> T>,
}

impl<T> Clone for CalendarExpr<T> {
    fn clone(&self) -> Self {
        Self {
            unit: self.unit,
            subtract: self.subtract,
            args: self.args.clone(),
            _marker: PhantomData,
        }
    }
}

impl<T> CalendarExpr<T> {
    fn new(unit: CalendarUnit, subtract: bool, timestamp: Expr<T>, amount: Expr<i64>) -> Self {
        Self {
            unit,
            subtract,
            args: FunctionArgs::new((timestamp, amount)),
            _marker: PhantomData,
        }
    }
}

impl<T: 'static> DbExpression<T> for CalendarExpr<T> {
    fn args(&self) -> FunctionArgs {
        self.args.clone()
    }

    fn render(&self, ctx: &mut ExprRenderCtx<'_>) -> Result<(), QueryError> {
        ctx.push_sql("TIMESTAMPADD(");
        ctx.push_sql(self.unit.sql());
        ctx.push_sql(", ");
        if self.subtract {
            ctx.push_sql("-(");
        }
        ctx.push_arg(1)?;
        if self.subtract {
            ctx.push_sql(")");
        }
        ctx.push_sql(", ");
        ctx.push_arg(0)?;
        ctx.push_sql(")");
        Ok(())
    }
}

struct MySqlDiffExpr<R> {
    unit: CalendarUnit,
    args: FunctionArgs,
    _marker: PhantomData<fn() -> R>,
}

impl<R> Clone for MySqlDiffExpr<R> {
    fn clone(&self) -> Self {
        Self {
            unit: self.unit,
            args: self.args.clone(),
            _marker: PhantomData,
        }
    }
}

impl<R> MySqlDiffExpr<R> {
    fn new<L, T>(unit: CalendarUnit, start: Expr<L>, end: Expr<T>) -> Self {
        Self {
            unit,
            args: FunctionArgs::new((start, end)),
            _marker: PhantomData,
        }
    }
}

impl<R: 'static> DbExpression<R> for MySqlDiffExpr<R> {
    fn args(&self) -> FunctionArgs {
        self.args.clone()
    }

    fn render(&self, ctx: &mut ExprRenderCtx<'_>) -> Result<(), QueryError> {
        ctx.push_sql("TIMESTAMPDIFF(");
        ctx.push_sql(self.unit.sql());
        ctx.push_sql(", ");
        ctx.push_arg(0)?;
        ctx.push_sql(", ");
        ctx.push_arg(1)?;
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
    fn new<T>(timestamp: Expr<T>, from_zone: String, to_zone: String) -> Self {
        Self {
            args: FunctionArgs::new((timestamp, val(from_zone), val(to_zone))),
            _marker: PhantomData,
        }
    }
}

impl<R: 'static> DbExpression<R> for TimeZoneExpr<R> {
    fn args(&self) -> FunctionArgs {
        self.args.clone()
    }

    fn render(&self, ctx: &mut ExprRenderCtx<'_>) -> Result<(), QueryError> {
        ctx.push_sql("CONVERT_TZ(");
        ctx.push_arg(0)?;
        ctx.push_sql(", ");
        ctx.push_arg(1)?;
        ctx.push_sql(", ");
        ctx.push_arg(2)?;
        ctx.push_sql(")");
        Ok(())
    }
}

#[derive(Clone)]
struct DateFormatExpr {
    args: FunctionArgs,
}

impl DateFormatExpr {
    fn new<T>(timestamp: Expr<T>, format: String) -> Self {
        Self {
            args: FunctionArgs::new((timestamp, val(format))),
        }
    }
}

impl DbExpression<String> for DateFormatExpr {
    fn args(&self) -> FunctionArgs {
        self.args.clone()
    }

    fn render(&self, ctx: &mut ExprRenderCtx<'_>) -> Result<(), QueryError> {
        ctx.push_sql("DATE_FORMAT(");
        ctx.push_arg(0)?;
        ctx.push_sql(", ");
        ctx.push_arg(1)?;
        ctx.push_sql(")");
        Ok(())
    }
}
