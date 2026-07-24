//! Portable UTC datetime expressions shared by every database backend.

use std::marker::PhantomData;

use crate::QueryError;
use crate::placeholders::Dialect;
use crate::queries::funcs::custom;
use crate::queries::{DbExpression, ExprRenderCtx, FunctionArgs};
use crate::queries::{Expr, IntoExpr, val};

use super::duration::FixedDuration;
use super::traits::{
    NonNullSqlDate, NonNullSqlTimestamp, SqlDatePartSource, SqlTimePartSource, SqlTimestamp,
    SqlTimestampPair,
};

/// Returns the database statement timestamp as the selected Rust timestamp type.
pub fn now<T>() -> Expr<T>
where
    T: NonNullSqlTimestamp,
{
    custom(CurrentExpr::<T>::new(CurrentKind::Timestamp))
}

/// Returns the current UTC date as the selected Rust date type.
pub fn current_date<T>() -> Expr<T>
where
    T: NonNullSqlDate,
{
    custom(CurrentExpr::<T>::new(CurrentKind::Date))
}

/// Converts a UTC timestamp expression to its associated date type.
pub fn date<T>(timestamp: impl IntoExpr<T>) -> Expr<T::Date>
where
    T: SqlTimestamp,
{
    custom(DateExpr::<T::Date>::new(timestamp.into_expr()))
}

macro_rules! date_part_fn {
    ($name:ident, $part:ident, $doc:literal) => {
        #[doc = $doc]
        pub fn $name<T>(value: impl IntoExpr<T>) -> Expr<T::Part>
        where
            T: SqlDatePartSource,
        {
            custom(PartExpr::<T::Part>::new(
                DatePart::$part,
                value.into_expr(),
                is_timestamp::<T>(),
            ))
        }
    };
}

date_part_fn!(extract_year, Year, "Extracts the Gregorian calendar year.");
date_part_fn!(
    extract_iso_year,
    IsoYear,
    "Extracts the ISO-8601 week-numbering year."
);
date_part_fn!(
    extract_quarter,
    Quarter,
    "Extracts the calendar quarter from 1 through 4."
);
date_part_fn!(
    extract_month,
    Month,
    "Extracts the month from 1 through 12."
);
date_part_fn!(
    extract_iso_week,
    IsoWeek,
    "Extracts the ISO-8601 week number."
);
date_part_fn!(extract_day, Day, "Extracts the day of the month.");
date_part_fn!(
    extract_ordinal_day,
    OrdinalDay,
    "Extracts the one-based day of the year."
);
date_part_fn!(
    extract_iso_weekday,
    IsoWeekday,
    "Extracts the ISO weekday from Monday 1 through Sunday 7."
);

macro_rules! time_part_fn {
    ($name:ident, $part:ident, $doc:literal) => {
        #[doc = $doc]
        pub fn $name<T>(value: impl IntoExpr<T>) -> Expr<T::Part>
        where
            T: SqlTimePartSource,
        {
            custom(PartExpr::<T::Part>::new(
                DatePart::$part,
                value.into_expr(),
                true,
            ))
        }
    };
}

time_part_fn!(
    extract_hour,
    Hour,
    "Extracts the UTC hour from 0 through 23."
);
time_part_fn!(
    extract_minute,
    Minute,
    "Extracts the minute from 0 through 59."
);
time_part_fn!(
    extract_second,
    Second,
    "Extracts the whole second from 0 through 59."
);

macro_rules! trunc_fn {
    ($name:ident, $unit:ident, $doc:literal) => {
        #[doc = $doc]
        pub fn $name<T>(timestamp: impl IntoExpr<T>) -> Expr<T>
        where
            T: SqlTimestamp,
        {
            custom(TruncExpr::<T>::new(timestamp.into_expr(), TruncUnit::$unit))
        }
    };
}

trunc_fn!(
    trunc_year,
    Year,
    "Truncates a UTC timestamp to the start of its year."
);
trunc_fn!(
    trunc_quarter,
    Quarter,
    "Truncates a UTC timestamp to the start of its quarter."
);
trunc_fn!(
    trunc_month,
    Month,
    "Truncates a UTC timestamp to the start of its month."
);
trunc_fn!(
    trunc_week,
    Week,
    "Truncates a UTC timestamp to Monday of its ISO week."
);
trunc_fn!(
    trunc_day,
    Day,
    "Truncates a UTC timestamp to the start of its day."
);
trunc_fn!(
    trunc_hour,
    Hour,
    "Truncates a UTC timestamp to the start of its hour."
);
trunc_fn!(
    trunc_minute,
    Minute,
    "Truncates a UTC timestamp to the start of its minute."
);
trunc_fn!(
    trunc_second,
    Second,
    "Removes the fractional part of a UTC timestamp."
);

/// Adds an exact whole-millisecond Rust duration to a UTC timestamp expression.
pub fn add<T, D>(timestamp: impl IntoExpr<T>, duration: D) -> Expr<T>
where
    T: SqlTimestamp,
    D: FixedDuration,
{
    custom(DurationExpr::<T>::new(
        timestamp.into_expr(),
        duration.checked_sql_microseconds(),
    ))
}

/// Subtracts an exact whole-millisecond Rust duration from a UTC timestamp expression.
pub fn subtract<T, D>(timestamp: impl IntoExpr<T>, duration: D) -> Expr<T>
where
    T: SqlTimestamp,
    D: FixedDuration,
{
    let micros = duration.checked_sql_microseconds().and_then(|value| {
        value.checked_neg().ok_or(QueryError::DateTimeOverflow {
            rust_type: std::any::type_name::<D>(),
        })
    });
    custom(DurationExpr::<T>::new(timestamp.into_expr(), micros))
}

macro_rules! diff_fn {
    ($name:ident, $divisor:expr, $doc:literal) => {
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
                DifferenceExpr::<<L as SqlTimestampPair<R>>::Difference>::new(
                    $divisor,
                    start.into_expr(),
                    end.into_expr(),
                ),
            )
        }
    };
}

diff_fn!(
    diff_milliseconds,
    1_000_i64,
    "Returns complete elapsed milliseconds as `end - start`."
);
diff_fn!(
    diff_seconds,
    1_000_000_i64,
    "Returns complete elapsed seconds as `end - start`."
);
diff_fn!(
    diff_minutes,
    60_000_000_i64,
    "Returns complete elapsed minutes as `end - start`."
);
diff_fn!(
    diff_hours,
    3_600_000_000_i64,
    "Returns complete elapsed hours as `end - start`."
);
diff_fn!(
    diff_days,
    86_400_000_000_i64,
    "Returns complete fixed 24-hour days as `end - start`."
);

#[derive(Clone, Copy)]
enum CurrentKind {
    Timestamp,
    Date,
}

struct CurrentExpr<T> {
    kind: CurrentKind,
    family: SqliteFamily,
    _marker: PhantomData<fn() -> T>,
}

impl<T> Clone for CurrentExpr<T> {
    fn clone(&self) -> Self {
        Self {
            kind: self.kind,
            family: self.family,
            _marker: PhantomData,
        }
    }
}

impl<T> CurrentExpr<T> {
    fn new(kind: CurrentKind) -> Self {
        Self {
            kind,
            family: sqlite_family::<T>(),
            _marker: PhantomData,
        }
    }
}

impl<T: 'static> DbExpression<T> for CurrentExpr<T> {
    fn render(&self, ctx: &mut ExprRenderCtx<'_>) -> Result<(), QueryError> {
        let sql = match (ctx.dialect(), self.kind) {
            (Dialect::Postgres, CurrentKind::Timestamp) => "STATEMENT_TIMESTAMP()".to_string(),
            (Dialect::Postgres, CurrentKind::Date) => {
                "CAST(STATEMENT_TIMESTAMP() AT TIME ZONE 'UTC' AS DATE)".to_string()
            }
            (Dialect::Mysql | Dialect::Mariadb, CurrentKind::Timestamp) => {
                "UTC_TIMESTAMP(6)".to_string()
            }
            (Dialect::Mysql | Dialect::Mariadb, CurrentKind::Date) => "UTC_DATE()".to_string(),
            (Dialect::Sqlite, CurrentKind::Timestamp) => sqlite_format("'now'", self.family, true),
            (Dialect::Sqlite, CurrentKind::Date) => "date('now')".to_string(),
        };
        ctx.push_sql(&sql);
        Ok(())
    }
}

struct DateExpr<R> {
    args: FunctionArgs,
    _marker: PhantomData<fn() -> R>,
}

impl<R> Clone for DateExpr<R> {
    fn clone(&self) -> Self {
        Self {
            args: self.args.clone(),
            _marker: PhantomData,
        }
    }
}

impl<R> DateExpr<R> {
    fn new<T>(timestamp: Expr<T>) -> Self {
        Self {
            args: FunctionArgs::new((timestamp,)),
            _marker: PhantomData,
        }
    }
}

impl<R: 'static> DbExpression<R> for DateExpr<R> {
    fn args(&self) -> FunctionArgs {
        self.args.clone()
    }

    fn render(&self, ctx: &mut ExprRenderCtx<'_>) -> Result<(), QueryError> {
        let (prefix, suffix) = match ctx.dialect() {
            Dialect::Postgres => ("CAST((", " AT TIME ZONE 'UTC') AS DATE)"),
            Dialect::Mysql | Dialect::Mariadb => ("DATE(", ")"),
            Dialect::Sqlite => ("date(", ")"),
        };
        push_arg_wrapped(ctx, prefix, 0, suffix)
    }
}

#[derive(Clone, Copy)]
enum DatePart {
    Year,
    IsoYear,
    Quarter,
    Month,
    IsoWeek,
    Day,
    OrdinalDay,
    IsoWeekday,
    Hour,
    Minute,
    Second,
}

struct PartExpr<R> {
    part: DatePart,
    args: FunctionArgs,
    timestamp: bool,
    _marker: PhantomData<fn() -> R>,
}

impl<R> Clone for PartExpr<R> {
    fn clone(&self) -> Self {
        Self {
            part: self.part,
            args: self.args.clone(),
            timestamp: self.timestamp,
            _marker: PhantomData,
        }
    }
}

impl<R> PartExpr<R> {
    fn new<T>(part: DatePart, value: Expr<T>, timestamp: bool) -> Self {
        Self {
            part,
            args: FunctionArgs::new((value,)),
            timestamp,
            _marker: PhantomData,
        }
    }
}

impl<R: 'static> DbExpression<R> for PartExpr<R> {
    fn args(&self) -> FunctionArgs {
        self.args.clone()
    }

    fn render(&self, ctx: &mut ExprRenderCtx<'_>) -> Result<(), QueryError> {
        match ctx.dialect() {
            Dialect::Postgres => push_pg_part(ctx, self.part, self.timestamp),
            Dialect::Mysql | Dialect::Mariadb => push_mysql_part(ctx, self.part),
            Dialect::Sqlite => push_sqlite_part(ctx, self.part),
        }
    }
}

#[derive(Clone, Copy)]
enum TruncUnit {
    Year,
    Quarter,
    Month,
    Week,
    Day,
    Hour,
    Minute,
    Second,
}

struct TruncExpr<T> {
    unit: TruncUnit,
    family: SqliteFamily,
    args: FunctionArgs,
    _marker: PhantomData<fn() -> T>,
}

impl<T> Clone for TruncExpr<T> {
    fn clone(&self) -> Self {
        Self {
            unit: self.unit,
            family: self.family,
            args: self.args.clone(),
            _marker: PhantomData,
        }
    }
}

impl<T> TruncExpr<T> {
    fn new(timestamp: Expr<T>, unit: TruncUnit) -> Self {
        Self {
            unit,
            family: sqlite_family::<T>(),
            args: FunctionArgs::new((timestamp,)),
            _marker: PhantomData,
        }
    }
}

impl<T: 'static> DbExpression<T> for TruncExpr<T> {
    fn args(&self) -> FunctionArgs {
        self.args.clone()
    }

    fn render(&self, ctx: &mut ExprRenderCtx<'_>) -> Result<(), QueryError> {
        match ctx.dialect() {
            Dialect::Postgres => push_pg_trunc(ctx, self.unit),
            Dialect::Mysql | Dialect::Mariadb => push_mysql_trunc(ctx, self.unit),
            Dialect::Sqlite => push_sqlite_trunc(ctx, self.unit, self.family),
        }
    }
}

struct DurationExpr<T> {
    conversion: Result<i64, QueryError>,
    family: SqliteFamily,
    args: FunctionArgs,
    _marker: PhantomData<fn() -> T>,
}

impl<T> Clone for DurationExpr<T> {
    fn clone(&self) -> Self {
        Self {
            conversion: self.conversion.clone(),
            family: self.family,
            args: self.args.clone(),
            _marker: PhantomData,
        }
    }
}

impl<T> DurationExpr<T> {
    fn new(timestamp: Expr<T>, conversion: Result<i64, QueryError>) -> Self {
        let args = match conversion {
            Ok(micros) => FunctionArgs::new((timestamp, val(micros))),
            Err(_) => FunctionArgs::new((timestamp,)),
        };
        Self {
            conversion,
            family: sqlite_family::<T>(),
            args,
            _marker: PhantomData,
        }
    }
}

impl<T: 'static> DbExpression<T> for DurationExpr<T> {
    fn args(&self) -> FunctionArgs {
        self.args.clone()
    }

    fn validate(&self, _dialect: crate::SqlDialect) -> Result<(), QueryError> {
        self.conversion.clone().map(|_| ())
    }

    fn render(&self, ctx: &mut ExprRenderCtx<'_>) -> Result<(), QueryError> {
        match ctx.dialect() {
            Dialect::Postgres => push_pg_duration(ctx),
            Dialect::Mysql | Dialect::Mariadb => push_mysql_duration(ctx),
            Dialect::Sqlite => push_sqlite_duration(ctx, self.family),
        }
    }
}

struct DifferenceExpr<R> {
    divisor: i64,
    args: FunctionArgs,
    _marker: PhantomData<fn() -> R>,
}

impl<R> Clone for DifferenceExpr<R> {
    fn clone(&self) -> Self {
        Self {
            divisor: self.divisor,
            args: self.args.clone(),
            _marker: PhantomData,
        }
    }
}

impl<R> DifferenceExpr<R> {
    fn new<L, T>(divisor: i64, start: Expr<L>, end: Expr<T>) -> Self {
        Self {
            divisor,
            args: FunctionArgs::new((start, end)),
            _marker: PhantomData,
        }
    }
}

impl<R: 'static> DbExpression<R> for DifferenceExpr<R> {
    fn args(&self) -> FunctionArgs {
        self.args.clone()
    }

    fn render(&self, ctx: &mut ExprRenderCtx<'_>) -> Result<(), QueryError> {
        match ctx.dialect() {
            Dialect::Postgres => push_pg_difference(ctx, self.divisor),
            Dialect::Mysql | Dialect::Mariadb => push_mysql_difference(ctx, self.divisor),
            Dialect::Sqlite => push_sqlite_difference(ctx, self.divisor),
        }
    }
}

fn date_part_field(part: DatePart) -> &'static str {
    match part {
        DatePart::Year => "YEAR",
        DatePart::IsoYear => "ISOYEAR",
        DatePart::Quarter => "QUARTER",
        DatePart::Month => "MONTH",
        DatePart::IsoWeek => "WEEK",
        DatePart::Day => "DAY",
        DatePart::OrdinalDay => "DOY",
        DatePart::IsoWeekday => "ISODOW",
        DatePart::Hour => "HOUR",
        DatePart::Minute => "MINUTE",
        DatePart::Second => "SECOND",
    }
}

fn push_pg_part(
    ctx: &mut ExprRenderCtx<'_>,
    part: DatePart,
    timestamp: bool,
) -> Result<(), QueryError> {
    ctx.push_sql(if matches!(part, DatePart::Second) {
        "CAST(FLOOR(EXTRACT("
    } else {
        "CAST(EXTRACT("
    });
    ctx.push_sql(date_part_field(part));
    ctx.push_sql(" FROM ");
    if timestamp {
        ctx.push_sql("(");
    }
    ctx.push_arg(0)?;
    if timestamp {
        ctx.push_sql(" AT TIME ZONE 'UTC')");
    }
    ctx.push_sql(if matches!(part, DatePart::Second) {
        ")) AS INTEGER)"
    } else {
        ") AS INTEGER)"
    });
    Ok(())
}

fn push_mysql_part(ctx: &mut ExprRenderCtx<'_>, part: DatePart) -> Result<(), QueryError> {
    let (prefix, suffix) = match part {
        DatePart::Year => ("CAST((YEAR(", ")) AS SIGNED)"),
        DatePart::IsoYear => ("CAST((FLOOR(YEARWEEK(", ", 3) / 100)) AS SIGNED)"),
        DatePart::Quarter => ("CAST((QUARTER(", ")) AS SIGNED)"),
        DatePart::Month => ("CAST((MONTH(", ")) AS SIGNED)"),
        DatePart::IsoWeek => ("CAST((WEEK(", ", 3)) AS SIGNED)"),
        DatePart::Day => ("CAST((DAYOFMONTH(", ")) AS SIGNED)"),
        DatePart::OrdinalDay => ("CAST((DAYOFYEAR(", ")) AS SIGNED)"),
        DatePart::IsoWeekday => ("CAST((WEEKDAY(", ") + 1) AS SIGNED)"),
        DatePart::Hour => ("CAST((HOUR(", ")) AS SIGNED)"),
        DatePart::Minute => ("CAST((MINUTE(", ")) AS SIGNED)"),
        DatePart::Second => ("CAST((SECOND(", ")) AS SIGNED)"),
    };
    push_arg_wrapped(ctx, prefix, 0, suffix)
}

fn push_sqlite_part(ctx: &mut ExprRenderCtx<'_>, part: DatePart) -> Result<(), QueryError> {
    let (prefix, suffix) = match part {
        DatePart::Year => ("CAST((strftime('%Y', ", ")) AS INTEGER)"),
        DatePart::IsoYear => (
            "CAST((strftime('%Y', date(",
            ", '-3 days', 'weekday 4'))) AS INTEGER)",
        ),
        DatePart::Quarter => (
            "CAST((((CAST(strftime('%m', ",
            ") AS INTEGER) - 1) / 3) + 1) AS INTEGER)",
        ),
        DatePart::Month => ("CAST((strftime('%m', ", ")) AS INTEGER)"),
        DatePart::IsoWeek => (
            "CAST((((CAST(strftime('%j', date(",
            ", '-3 days', 'weekday 4')) AS INTEGER) - 1) / 7) + 1) AS INTEGER)",
        ),
        DatePart::Day => ("CAST((strftime('%d', ", ")) AS INTEGER)"),
        DatePart::OrdinalDay => ("CAST((strftime('%j', ", ")) AS INTEGER)"),
        DatePart::IsoWeekday => (
            "CAST((((CAST(strftime('%w', ",
            ") AS INTEGER) + 6) % 7) + 1) AS INTEGER)",
        ),
        DatePart::Hour => ("CAST((strftime('%H', ", ")) AS INTEGER)"),
        DatePart::Minute => ("CAST((strftime('%M', ", ")) AS INTEGER)"),
        DatePart::Second => ("CAST((strftime('%S', ", ")) AS INTEGER)"),
    };
    push_arg_wrapped(ctx, prefix, 0, suffix)
}

fn push_pg_trunc(ctx: &mut ExprRenderCtx<'_>, unit: TruncUnit) -> Result<(), QueryError> {
    ctx.push_sql("(date_trunc('");
    ctx.push_sql(trunc_name(unit));
    ctx.push_sql("', ");
    ctx.push_arg(0)?;
    ctx.push_sql(" AT TIME ZONE 'UTC') AT TIME ZONE 'UTC')");
    Ok(())
}

fn push_mysql_trunc(ctx: &mut ExprRenderCtx<'_>, unit: TruncUnit) -> Result<(), QueryError> {
    match unit {
        TruncUnit::Year => push_arg_wrapped(ctx, "TIMESTAMP(MAKEDATE(YEAR(", 0, "), 1))"),
        TruncUnit::Quarter => {
            ctx.push_sql("TIMESTAMP(DATE_ADD(MAKEDATE(YEAR(");
            ctx.push_arg(0)?;
            ctx.push_sql("), 1), INTERVAL ((QUARTER(");
            ctx.push_arg(0)?;
            ctx.push_sql(") - 1) * 3) MONTH))");
            Ok(())
        }
        TruncUnit::Month => push_arg_wrapped(ctx, "TIMESTAMP(DATE_FORMAT(", 0, ", '%Y-%m-01'))"),
        TruncUnit::Week => {
            ctx.push_sql("TIMESTAMP(DATE_SUB(DATE(");
            ctx.push_arg(0)?;
            ctx.push_sql("), INTERVAL WEEKDAY(");
            ctx.push_arg(0)?;
            ctx.push_sql(") DAY))");
            Ok(())
        }
        TruncUnit::Day => push_arg_wrapped(ctx, "TIMESTAMP(DATE(", 0, "))"),
        TruncUnit::Hour => {
            push_arg_wrapped(ctx, "TIMESTAMP(DATE_FORMAT(", 0, ", '%Y-%m-%d %H:00:00'))")
        }
        TruncUnit::Minute => {
            push_arg_wrapped(ctx, "TIMESTAMP(DATE_FORMAT(", 0, ", '%Y-%m-%d %H:%i:00'))")
        }
        TruncUnit::Second => {
            push_arg_wrapped(ctx, "TIMESTAMP(DATE_FORMAT(", 0, ", '%Y-%m-%d %H:%i:%s'))")
        }
    }
}

fn push_sqlite_trunc(
    ctx: &mut ExprRenderCtx<'_>,
    unit: TruncUnit,
    family: SqliteFamily,
) -> Result<(), QueryError> {
    push_sqlite_format_start(ctx, family, false);
    match unit {
        TruncUnit::Year => push_arg_wrapped(ctx, "datetime(", 0, ", 'start of year')")?,
        TruncUnit::Quarter => push_sqlite_quarter(ctx)?,
        TruncUnit::Month => push_arg_wrapped(ctx, "datetime(", 0, ", 'start of month')")?,
        TruncUnit::Week => push_sqlite_week(ctx)?,
        TruncUnit::Day => push_arg_wrapped(ctx, "datetime(", 0, ", 'start of day')")?,
        TruncUnit::Hour => push_arg_wrapped(ctx, "strftime('%Y-%m-%d %H:00:00', ", 0, ")")?,
        TruncUnit::Minute => push_arg_wrapped(ctx, "strftime('%Y-%m-%d %H:%M:00', ", 0, ")")?,
        TruncUnit::Second => push_arg_wrapped(ctx, "strftime('%Y-%m-%d %H:%M:%S', ", 0, ")")?,
    }
    push_sqlite_format_end(ctx, family);
    Ok(())
}

fn push_sqlite_quarter(ctx: &mut ExprRenderCtx<'_>) -> Result<(), QueryError> {
    ctx.push_sql("datetime(");
    ctx.push_arg(0)?;
    ctx.push_sql(", printf('-%d months', (CAST(strftime('%m', ");
    ctx.push_arg(0)?;
    ctx.push_sql(") AS INTEGER) - 1) % 3), 'start of month')");
    Ok(())
}

fn push_sqlite_week(ctx: &mut ExprRenderCtx<'_>) -> Result<(), QueryError> {
    ctx.push_sql("datetime(");
    ctx.push_arg(0)?;
    ctx.push_sql(", printf('-%d days', (CAST(strftime('%w', ");
    ctx.push_arg(0)?;
    ctx.push_sql(") AS INTEGER) + 6) % 7), 'start of day')");
    Ok(())
}

fn push_pg_duration(ctx: &mut ExprRenderCtx<'_>) -> Result<(), QueryError> {
    ctx.push_sql("(");
    ctx.push_arg(0)?;
    ctx.push_sql(" + (");
    ctx.push_arg(1)?;
    ctx.push_sql(" * INTERVAL '1 microsecond'))");
    Ok(())
}

fn push_mysql_duration(ctx: &mut ExprRenderCtx<'_>) -> Result<(), QueryError> {
    ctx.push_sql("TIMESTAMPADD(MICROSECOND, ");
    ctx.push_arg(1)?;
    ctx.push_sql(", ");
    ctx.push_arg(0)?;
    ctx.push_sql(")");
    Ok(())
}

fn push_sqlite_duration(
    ctx: &mut ExprRenderCtx<'_>,
    family: SqliteFamily,
) -> Result<(), QueryError> {
    push_sqlite_format_start(ctx, family, true);
    ctx.push_sql("(julianday(");
    ctx.push_arg(0)?;
    ctx.push_sql(") + (");
    ctx.push_arg(1)?;
    ctx.push_sql(" / 86400000000.0))");
    push_sqlite_format_end(ctx, family);
    Ok(())
}

fn push_pg_difference(ctx: &mut ExprRenderCtx<'_>, divisor: i64) -> Result<(), QueryError> {
    ctx.push_sql("CAST(TRUNC((EXTRACT(EPOCH FROM (");
    ctx.push_arg(1)?;
    ctx.push_sql(" - ");
    ctx.push_arg(0)?;
    ctx.push_sql(")) * 1000000) / ");
    ctx.push_sql(&divisor.to_string());
    ctx.push_sql(") AS BIGINT)");
    Ok(())
}

fn push_mysql_difference(ctx: &mut ExprRenderCtx<'_>, divisor: i64) -> Result<(), QueryError> {
    ctx.push_sql("CAST(TRUNCATE(TIMESTAMPDIFF(MICROSECOND, ");
    ctx.push_arg(0)?;
    ctx.push_sql(", ");
    ctx.push_arg(1)?;
    ctx.push_sql(") / ");
    ctx.push_sql(&divisor.to_string());
    ctx.push_sql(", 0) AS SIGNED)");
    Ok(())
}

fn push_sqlite_difference(ctx: &mut ExprRenderCtx<'_>, divisor: i64) -> Result<(), QueryError> {
    ctx.push_sql("CAST(((");
    push_sqlite_epoch_micros(ctx, 1)?;
    ctx.push_sql(" - ");
    push_sqlite_epoch_micros(ctx, 0)?;
    ctx.push_sql(") / ");
    ctx.push_sql(&divisor.to_string());
    ctx.push_sql(") AS INTEGER)");
    Ok(())
}

fn push_sqlite_epoch_micros(ctx: &mut ExprRenderCtx<'_>, index: usize) -> Result<(), QueryError> {
    ctx.push_sql("(CAST(ROUND((julianday(");
    ctx.push_arg(index)?;
    ctx.push_sql(") - 2440587.5) * 86400000.0) AS INTEGER) * 1000)");
    Ok(())
}

fn push_arg_wrapped(
    ctx: &mut ExprRenderCtx<'_>,
    prefix: &str,
    index: usize,
    suffix: &str,
) -> Result<(), QueryError> {
    ctx.push_sql(prefix);
    ctx.push_arg(index)?;
    ctx.push_sql(suffix);
    Ok(())
}

pub(super) fn push_sqlite_format_start(
    ctx: &mut ExprRenderCtx<'_>,
    _family: SqliteFamily,
    milliseconds: bool,
) {
    let format = if milliseconds {
        "%Y-%m-%dT%H:%M:%f"
    } else {
        "%Y-%m-%dT%H:%M:%S"
    };
    ctx.push_sql("(strftime('");
    ctx.push_sql(format);
    ctx.push_sql("', ");
}

pub(super) fn push_sqlite_format_end(ctx: &mut ExprRenderCtx<'_>, family: SqliteFamily) {
    ctx.push_sql(") || '");
    ctx.push_sql(match family {
        SqliteFamily::Chrono => "+00:00",
        SqliteFamily::Time => "Z",
    });
    ctx.push_sql("')");
}

/// Returns the trusted PostgreSQL `date_trunc` keyword for a public helper.
fn trunc_name(unit: TruncUnit) -> &'static str {
    match unit {
        TruncUnit::Year => "year",
        TruncUnit::Quarter => "quarter",
        TruncUnit::Month => "month",
        TruncUnit::Week => "week",
        TruncUnit::Day => "day",
        TruncUnit::Hour => "hour",
        TruncUnit::Minute => "minute",
        TruncUnit::Second => "second",
    }
}

#[derive(Clone, Copy)]
pub(super) enum SqliteFamily {
    Chrono,
    Time,
}

pub(super) fn sqlite_family<T>() -> SqliteFamily {
    if std::any::type_name::<T>().contains("time::offset_date_time::OffsetDateTime") {
        SqliteFamily::Time
    } else {
        SqliteFamily::Chrono
    }
}

/// Formats a SQLite value in the canonical representation for one Rust family.
pub(super) fn sqlite_format(source: &str, family: SqliteFamily, milliseconds: bool) -> String {
    let format = if milliseconds {
        "%Y-%m-%dT%H:%M:%f"
    } else {
        "%Y-%m-%dT%H:%M:%S"
    };
    let suffix = match family {
        SqliteFamily::Chrono => "+00:00",
        SqliteFamily::Time => "Z",
    };
    format!("(strftime('{format}', {source}) || '{suffix}')")
}

fn is_timestamp<T>() -> bool {
    let name = std::any::type_name::<T>();
    name.contains("DateTime") || name.contains("OffsetDateTime")
}
