use mool as db;
use mool::Model;

#[derive(Debug, Clone, db::Model)]
#[table(name = "temporal_rows")]
struct TemporalRow {
    id: i64,
    created_at: chrono::DateTime<chrono::Utc>,
    optional_at: Option<chrono::DateTime<chrono::Utc>>,
    created_on: chrono::NaiveDate,
    local_at: chrono::NaiveDateTime,
}

#[cfg(feature = "time")]
#[derive(Debug, Clone, db::Model)]
#[table(name = "time_temporal_rows")]
struct TimeTemporalRow {
    id: i64,
    created_at: time::OffsetDateTime,
    optional_at: Option<time::OffsetDateTime>,
    created_on: time::Date,
    local_at: time::PrimitiveDateTime,
}

#[cfg(feature = "postgres")]
const NOW_SQL: &str = "SELECT STATEMENT_TIMESTAMP() FROM temporal_rows LIMIT 1";
#[cfg(any(feature = "mysql", feature = "mariadb"))]
const NOW_SQL: &str = "SELECT UTC_TIMESTAMP(6) FROM temporal_rows LIMIT 1";
#[cfg(feature = "sqlite")]
const NOW_SQL: &str =
    "SELECT (strftime('%Y-%m-%dT%H:%M:%f', 'now') || '+00:00') FROM temporal_rows LIMIT 1";

#[cfg(feature = "postgres")]
const CURRENT_DATE_SQL: &str =
    "SELECT CAST(STATEMENT_TIMESTAMP() AT TIME ZONE 'UTC' AS DATE) FROM temporal_rows LIMIT 1";
#[cfg(any(feature = "mysql", feature = "mariadb"))]
const CURRENT_DATE_SQL: &str = "SELECT UTC_DATE() FROM temporal_rows LIMIT 1";
#[cfg(feature = "sqlite")]
const CURRENT_DATE_SQL: &str = "SELECT date('now') FROM temporal_rows LIMIT 1";

#[cfg(feature = "postgres")]
const DATE_SQL: &str =
    "SELECT CAST((temporal_rows.created_at AT TIME ZONE 'UTC') AS DATE) FROM temporal_rows LIMIT 1";
#[cfg(any(feature = "mysql", feature = "mariadb"))]
const DATE_SQL: &str = "SELECT DATE(temporal_rows.created_at) FROM temporal_rows LIMIT 1";
#[cfg(feature = "sqlite")]
const DATE_SQL: &str = "SELECT date(temporal_rows.created_at) FROM temporal_rows LIMIT 1";

#[cfg(feature = "postgres")]
const YEAR_SQL: &str = "SELECT CAST(EXTRACT(YEAR FROM (temporal_rows.created_at AT TIME ZONE 'UTC')) AS INTEGER) FROM temporal_rows LIMIT 1";
#[cfg(any(feature = "mysql", feature = "mariadb"))]
const YEAR_SQL: &str =
    "SELECT CAST((YEAR(temporal_rows.created_at)) AS SIGNED) FROM temporal_rows LIMIT 1";
#[cfg(feature = "sqlite")]
const YEAR_SQL: &str =
    "SELECT CAST((strftime('%Y', temporal_rows.created_at)) AS INTEGER) FROM temporal_rows LIMIT 1";

#[cfg(feature = "postgres")]
const TRUNC_DAY_SQL: &str = "SELECT (date_trunc('day', temporal_rows.created_at AT TIME ZONE 'UTC') AT TIME ZONE 'UTC') FROM temporal_rows LIMIT 1";
#[cfg(any(feature = "mysql", feature = "mariadb"))]
const TRUNC_DAY_SQL: &str =
    "SELECT TIMESTAMP(DATE(temporal_rows.created_at)) FROM temporal_rows LIMIT 1";
#[cfg(feature = "sqlite")]
const TRUNC_DAY_SQL: &str = "SELECT (strftime('%Y-%m-%dT%H:%M:%S', datetime(temporal_rows.created_at, 'start of day')) || '+00:00') FROM temporal_rows LIMIT 1";

#[cfg(feature = "postgres")]
const ADD_SQL: &str = "SELECT (temporal_rows.created_at + ($1 * INTERVAL '1 microsecond')) FROM temporal_rows LIMIT 1";
#[cfg(any(feature = "mysql", feature = "mariadb"))]
const ADD_SQL: &str =
    "SELECT TIMESTAMPADD(MICROSECOND, ?, temporal_rows.created_at) FROM temporal_rows LIMIT 1";
#[cfg(feature = "sqlite")]
const ADD_SQL: &str = "SELECT (strftime('%Y-%m-%dT%H:%M:%f', (julianday(temporal_rows.created_at) + (? / 86400000000.0))) || '+00:00') FROM temporal_rows LIMIT 1";

#[cfg(feature = "postgres")]
const DIFF_SQL: &str = "SELECT CAST(TRUNC((EXTRACT(EPOCH FROM (temporal_rows.optional_at - temporal_rows.created_at)) * 1000000) / 1000000) AS BIGINT) FROM temporal_rows LIMIT 1";
#[cfg(any(feature = "mysql", feature = "mariadb"))]
const DIFF_SQL: &str = "SELECT CAST(TRUNCATE(TIMESTAMPDIFF(MICROSECOND, temporal_rows.created_at, temporal_rows.optional_at) / 1000000, 0) AS SIGNED) FROM temporal_rows LIMIT 1";
#[cfg(feature = "sqlite")]
const DIFF_SQL: &str = "SELECT CAST((((CAST(ROUND((julianday(temporal_rows.optional_at) - 2440587.5) * 86400000.0) AS INTEGER) * 1000) - (CAST(ROUND((julianday(temporal_rows.created_at) - 2440587.5) * 86400000.0) AS INTEGER) * 1000)) / 1000000) AS INTEGER) FROM temporal_rows LIMIT 1";

/// Verifies portable datetime expressions render exact backend SQL and bind metadata.
#[test]
fn portable_datetime_golden_sql_is_stable() {
    let rows = TemporalRow::table();

    let now = db::from(&rows)
        .scalar(db::funcs::datetime::now::<chrono::DateTime<chrono::Utc>>())
        .plan()
        .expect("portable statement timestamp");
    assert_eq!(now.sql, NOW_SQL);
    assert_eq!(now.total_bind_count, 0);

    let year = db::from(&rows)
        .scalar(db::funcs::datetime::extract_year(rows.created_at.clone()))
        .plan()
        .expect("portable year extraction");
    assert_eq!(year.sql, YEAR_SQL);
    assert_eq!(year.total_bind_count, 0);

    let day = db::from(&rows)
        .scalar(db::funcs::datetime::trunc_day(rows.created_at.clone()))
        .plan()
        .expect("portable day truncation");
    assert_eq!(day.sql, TRUNC_DAY_SQL);

    let added = db::from(&rows)
        .scalar(db::funcs::datetime::add(
            rows.created_at.clone(),
            chrono::TimeDelta::milliseconds(1_250),
        ))
        .plan()
        .expect("portable duration addition");
    assert_eq!(added.sql, ADD_SQL);
    assert_eq!(added.total_bind_count, 1);
    assert_eq!(added.dynamic_bind_count, 1);

    let difference = db::from(&rows)
        .scalar(db::funcs::datetime::diff_seconds(
            rows.created_at.clone(),
            rows.optional_at.clone(),
        ))
        .plan()
        .expect("nullable elapsed time");
    assert_eq!(difference.sql, DIFF_SQL);
    assert_eq!(difference.total_bind_count, 0);
}

/// Verifies current-date and timestamp-to-date helpers preserve typed date SQL.
#[test]
fn portable_date_conversion_sql_is_stable() {
    let rows = TemporalRow::table();
    let current = db::from(&rows)
        .scalar(db::funcs::datetime::current_date::<chrono::NaiveDate>())
        .plan()
        .expect("portable current date");
    let converted = db::from(&rows)
        .scalar(db::funcs::datetime::date(rows.created_at.clone()))
        .plan()
        .expect("portable timestamp date");

    assert_eq!(current.sql, CURRENT_DATE_SQL);
    assert_eq!(converted.sql, DATE_SQL);
    assert_eq!(current.total_bind_count, 0);
    assert_eq!(converted.total_bind_count, 0);
}

/// Verifies PostgreSQL whole-second extraction floors fractional seconds.
#[cfg(feature = "postgres")]
#[test]
fn postgres_second_extraction_does_not_round() {
    let rows = TemporalRow::table();
    let plan = db::from(&rows)
        .scalar(db::funcs::datetime::extract_second(rows.created_at.clone()))
        .plan()
        .expect("PostgreSQL second extraction");

    assert_eq!(
        plan.sql,
        "SELECT CAST(FLOOR(EXTRACT(SECOND FROM (temporal_rows.created_at AT TIME ZONE 'UTC'))) AS INTEGER) FROM temporal_rows LIMIT 1"
    );
}

/// Verifies every portable extraction and truncation helper composes into a scalar query.
#[test]
fn portable_datetime_helper_surface_is_composable() {
    let rows = TemporalRow::table();
    let date_parts = [
        db::funcs::datetime::extract_year(rows.created_at.clone()),
        db::funcs::datetime::extract_iso_year(rows.created_at.clone()),
        db::funcs::datetime::extract_quarter(rows.created_at.clone()),
        db::funcs::datetime::extract_month(rows.created_at.clone()),
        db::funcs::datetime::extract_iso_week(rows.created_at.clone()),
        db::funcs::datetime::extract_day(rows.created_at.clone()),
        db::funcs::datetime::extract_ordinal_day(rows.created_at.clone()),
        db::funcs::datetime::extract_iso_weekday(rows.created_at.clone()),
        db::funcs::datetime::extract_hour(rows.created_at.clone()),
        db::funcs::datetime::extract_minute(rows.created_at.clone()),
        db::funcs::datetime::extract_second(rows.created_at.clone()),
    ];
    for expression in date_parts {
        let plan = db::from(&rows)
            .scalar(expression)
            .plan()
            .expect("portable extraction query");
        assert_eq!(plan.total_bind_count, 0);
    }

    let truncations = [
        db::funcs::datetime::trunc_year(rows.created_at.clone()),
        db::funcs::datetime::trunc_quarter(rows.created_at.clone()),
        db::funcs::datetime::trunc_month(rows.created_at.clone()),
        db::funcs::datetime::trunc_week(rows.created_at.clone()),
        db::funcs::datetime::trunc_day(rows.created_at.clone()),
        db::funcs::datetime::trunc_hour(rows.created_at.clone()),
        db::funcs::datetime::trunc_minute(rows.created_at.clone()),
        db::funcs::datetime::trunc_second(rows.created_at.clone()),
    ];
    for expression in truncations {
        db::from(&rows)
            .scalar(expression)
            .plan()
            .expect("portable truncation query");
    }
}

/// Verifies duration adapters reject lossy and overflowing values during planning.
#[test]
fn portable_duration_validation_fails_before_execution() {
    let rows = TemporalRow::table();
    let precision = db::from(&rows)
        .scalar(db::funcs::datetime::add(
            rows.created_at.clone(),
            chrono::TimeDelta::microseconds(1),
        ))
        .plan()
        .expect_err("sub-millisecond duration must fail");
    assert!(matches!(
        precision,
        db::QueryError::DateTimePrecision { .. }
    ));

    let overflow = db::from(&rows)
        .scalar(db::funcs::datetime::add(
            rows.created_at.clone(),
            std::time::Duration::from_secs(u64::MAX),
        ))
        .plan()
        .expect_err("oversized duration must fail");
    assert!(matches!(overflow, db::QueryError::DateTimeOverflow { .. }));
}

/// Verifies Chrono, std, and Tokio durations use one portable expression contract.
#[test]
fn standard_duration_families_are_supported() {
    let rows = TemporalRow::table();
    let chrono_expr =
        db::funcs::datetime::subtract(rows.created_at.clone(), chrono::TimeDelta::seconds(-1));
    let std_expr =
        db::funcs::datetime::add(rows.created_at.clone(), std::time::Duration::from_secs(1));
    let tokio_expr =
        db::funcs::datetime::add(rows.created_at.clone(), tokio::time::Duration::from_secs(1));
    let zero_expr = db::funcs::datetime::add(rows.created_at.clone(), std::time::Duration::ZERO);
    let max_expr = db::funcs::datetime::add(
        rows.created_at.clone(),
        std::time::Duration::from_millis((i64::MAX / 1_000) as u64),
    );

    for plan in [
        db::from(&rows).scalar(chrono_expr).plan(),
        db::from(&rows).scalar(std_expr).plan(),
        db::from(&rows).scalar(tokio_expr).plan(),
        db::from(&rows).scalar(zero_expr).plan(),
        db::from(&rows).scalar(max_expr).plan(),
    ] {
        assert_eq!(plan.expect("supported duration").total_bind_count, 1);
    }
}

/// Verifies `time` crate expressions retain concrete result types and SQLite encoding family.
#[cfg(feature = "time")]
#[test]
fn time_crate_temporal_types_preserve_results() {
    let rows = TimeTemporalRow::table();
    let now = db::funcs::datetime::now::<time::OffsetDateTime>();
    let date = db::funcs::datetime::date(rows.created_at.clone());
    let optional = db::funcs::datetime::trunc_hour(rows.optional_at.clone());
    let added = db::funcs::datetime::add(rows.created_at.clone(), time::Duration::seconds(1));

    assert_expr::<time::OffsetDateTime>(&now);
    assert_expr::<time::Date>(&date);
    assert_expr::<Option<time::OffsetDateTime>>(&optional);
    assert_expr::<time::OffsetDateTime>(&added);

    db::from(&rows)
        .scalar(added)
        .plan()
        .expect("time crate duration query");
}

/// Verifies PostgreSQL-native interval, timezone, formatting, and clock expressions.
#[cfg(feature = "postgres")]
#[test]
fn postgres_datetime_extensions_render_exact_sql() {
    let rows = TemporalRow::table();
    let interval = db::sqlx::postgres::types::PgInterval {
        months: 1,
        days: 2,
        microseconds: 3_000,
    };

    let added = db::from(&rows)
        .scalar(db::funcs::postgres::datetime::add_interval(
            rows.created_at.clone(),
            interval,
        ))
        .plan()
        .expect("native interval addition");
    assert_eq!(
        added.sql,
        "SELECT (temporal_rows.created_at + $1) FROM temporal_rows LIMIT 1"
    );
    assert_eq!(added.total_bind_count, 1);

    let zoned = db::from(&rows)
        .scalar(db::funcs::postgres::datetime::at_time_zone(
            rows.created_at.clone(),
            "Asia/Kolkata",
        ))
        .plan()
        .expect("timezone conversion");
    assert_eq!(
        zoned.sql,
        "SELECT (temporal_rows.created_at AT TIME ZONE $1) FROM temporal_rows LIMIT 1"
    );

    let formatted = db::from(&rows)
        .scalar(db::funcs::postgres::datetime::to_char(
            rows.created_at.clone(),
            "YYYY-MM-DD",
        ))
        .plan()
        .expect("PostgreSQL timestamp formatting");
    assert_eq!(
        formatted.sql,
        "SELECT TO_CHAR(temporal_rows.created_at, $1) FROM temporal_rows LIMIT 1"
    );

    let transaction = db::from(&rows)
        .scalar(db::funcs::postgres::datetime::transaction_timestamp::<
            chrono::DateTime<chrono::Utc>,
        >())
        .plan()
        .expect("transaction timestamp");
    assert_eq!(
        transaction.sql,
        "SELECT TRANSACTION_TIMESTAMP() FROM temporal_rows LIMIT 1"
    );
}

/// Verifies MySQL-family calendar, timezone, native difference, and formatting expressions.
#[cfg(any(feature = "mysql", feature = "mariadb"))]
#[test]
fn mysql_family_datetime_extensions_render_exact_sql() {
    let rows = TemporalRow::table();
    #[cfg(feature = "mysql")]
    let calendar_expr =
        db::funcs::mysql::datetime::add_calendar_months(rows.created_at.clone(), db::val(2_i64));
    #[cfg(feature = "mariadb")]
    let calendar_expr =
        db::funcs::mariadb::datetime::add_calendar_months(rows.created_at.clone(), db::val(2_i64));

    let calendar = db::from(&rows)
        .scalar(calendar_expr)
        .plan()
        .expect("calendar month addition");
    assert_eq!(
        calendar.sql,
        "SELECT TIMESTAMPADD(MONTH, ?, temporal_rows.created_at) FROM temporal_rows LIMIT 1"
    );
    assert_eq!(calendar.total_bind_count, 1);

    #[cfg(feature = "mysql")]
    let zoned = db::funcs::mysql::datetime::convert_time_zone(
        rows.created_at.clone(),
        "+00:00",
        "Asia/Kolkata",
    );
    #[cfg(feature = "mariadb")]
    let zoned = db::funcs::mariadb::datetime::convert_time_zone(
        rows.created_at.clone(),
        "+00:00",
        "Asia/Kolkata",
    );
    let zoned = db::from(&rows)
        .scalar(zoned)
        .plan()
        .expect("MySQL-family timezone conversion");
    assert_eq!(
        zoned.sql,
        "SELECT CONVERT_TZ(temporal_rows.created_at, ?, ?) FROM temporal_rows LIMIT 1"
    );
    assert_eq!(zoned.total_bind_count, 2);
}

/// Verifies SQLite calendar ambiguity and low-level scalar datetime expressions.
#[cfg(feature = "sqlite")]
#[test]
fn sqlite_datetime_extensions_render_exact_sql() {
    let rows = TemporalRow::table();
    let calendar = db::from(&rows)
        .scalar(db::funcs::sqlite::datetime::add_calendar_months_floor(
            rows.created_at.clone(),
            db::val(1_i64),
        ))
        .plan()
        .expect("SQLite floor month arithmetic");
    assert_eq!(
        calendar.sql,
        "SELECT (strftime('%Y-%m-%dT%H:%M:%f', datetime(temporal_rows.created_at, printf('%+d months', ?), 'floor')) || '+00:00') FROM temporal_rows LIMIT 1"
    );

    let epoch = db::from(&rows)
        .scalar(db::funcs::sqlite::datetime::unixepoch(
            rows.created_at.clone(),
        ))
        .plan()
        .expect("SQLite Unix epoch conversion");
    assert_eq!(
        epoch.sql,
        "SELECT unixepoch(temporal_rows.created_at) FROM temporal_rows LIMIT 1"
    );
}

#[cfg(feature = "time")]
fn assert_expr<T>(_expression: &db::Expr<T>) {}

/// Verifies selected-backend schema inference recognizes standard temporal field types.
#[test]
fn model_schema_infers_temporal_column_types() {
    let table = <TemporalRow as db::schema::IntoTable>::into_table(&active_dialect());
    let column_type = |name: &str| {
        table
            .columns
            .iter()
            .find(|column| column.name == name)
            .map(|column| column.col_type.as_str())
    };

    #[cfg(feature = "postgres")]
    assert_eq!(column_type("created_at"), Some("timestamptz"));
    #[cfg(any(feature = "mysql", feature = "mariadb"))]
    assert_eq!(column_type("created_at"), Some("timestamp(6)"));
    #[cfg(feature = "sqlite")]
    assert_eq!(column_type("created_at"), Some("text"));
    assert_eq!(
        column_type("created_on"),
        Some(if cfg!(feature = "sqlite") {
            "text"
        } else {
            "date"
        })
    );
}

fn active_dialect() -> db::gaman::core::Dialect {
    #[cfg(feature = "postgres")]
    return db::gaman::core::Dialect::Postgres;
    #[cfg(feature = "sqlite")]
    return db::gaman::core::Dialect::Sqlite;
    #[cfg(feature = "mysql")]
    return db::gaman::core::Dialect::Mysql;
    #[cfg(feature = "mariadb")]
    return db::gaman::core::Dialect::Mariadb;
}
