use mool as db;
use mool::prelude::*;

#[derive(Debug, Clone, db::Model)]
#[table(name = "mool_datetime_live")]
struct LiveMoment {
    #[column(primary_key)]
    id: i64,
    happened_at: chrono::DateTime<chrono::Utc>,
}

/// Verifies generated datetime SQL executes with stable values and SQLx decoding.
#[db::sqlx::test]
#[ignore = "run through scripts/integration-tests.sh"]
async fn selected_backend_datetime_conformance(pool: db::backend::Pool) -> Result<(), db::DbError> {
    initialize_external_session(&pool).await?;
    let mut pool = db::DbPool::from_pool(pool);
    pool.execute(statement(create_table_sql())?).await?;

    let instant = chrono::DateTime::<chrono::Utc>::UNIX_EPOCH
        + chrono::TimeDelta::milliseconds(1_609_459_198_123);
    insert_fixture(&mut pool, instant).await?;
    assert_calendar_values(&mut pool).await?;
    assert_elapsed_values(&mut pool).await?;
    assert_time_decode(&pool, instant.timestamp()).await?;
    Ok(())
}

/// Verifies pools created by Mool initialize server-backed sessions in UTC.
#[cfg(any(feature = "postgres", feature = "mysql", feature = "mariadb"))]
#[tokio::test]
#[ignore = "run through scripts/integration-tests.sh"]
async fn mool_pool_initializes_server_sessions_in_utc() -> Result<(), db::DbError> {
    let conf = db::DbConf::from_env()?;
    let pool = db::DbPool::from_conf(&conf).await?;
    let zone: String = db::sqlx::query_scalar(session_timezone_sql())
        .fetch_one(pool.as_sqlx())
        .await
        .map_err(|error| db::DbError::from_sqlx(db::DbOperation::FetchScalar, error))?;

    assert!(matches!(zone.as_str(), "UTC" | "+00:00"));
    Ok(())
}

/// Inserts one canonical millisecond-precision UTC instant through Mool binding.
async fn insert_fixture(
    pool: &mut db::DbPool,
    instant: chrono::DateTime<chrono::Utc>,
) -> Result<(), db::DbError> {
    let insert =
        db::query("INSERT INTO mool_datetime_live (id, happened_at) VALUES (:id, :happened_at)")
            .bind("id", 1_i64)
            .bind("happened_at", instant)
            .to_statement()?;
    pool.execute(insert).await?;
    let count = pool
        .fetch_scalar::<i64>(statement("SELECT COUNT(*) FROM mool_datetime_live")?)
        .await?;
    assert_eq!(count, 1);
    Ok(())
}

/// Executes ISO extraction and UTC truncation against the live fixture.
async fn assert_calendar_values(pool: &mut db::DbPool) -> Result<(), db::DbError> {
    let moments = LiveMoment::table();
    let scope = || db::from(&moments).filter(moments.id.eq(db::val(1_i64)));
    let iso_year = scope()
        .scalar(db::funcs::datetime::extract_iso_year(
            moments.happened_at.clone(),
        ))
        .exec(&mut *pool)
        .await
        .map_err(|error| stage_error("extract_iso_year", error))?;
    let iso_week = scope()
        .scalar(db::funcs::datetime::extract_iso_week(
            moments.happened_at.clone(),
        ))
        .exec(&mut *pool)
        .await
        .map_err(|error| stage_error("extract_iso_week", error))?;
    let truncated = scope()
        .scalar(db::funcs::datetime::trunc_day(moments.happened_at.clone()))
        .exec(&mut *pool)
        .await
        .map_err(|error| stage_error("trunc_day", error))?;

    assert_eq!(iso_year, 2020);
    assert_eq!(iso_week, 53);
    assert_eq!(truncated.timestamp(), 1_609_372_800);
    Ok(())
}

/// Executes positive and negative fixed-duration differences against the fixture.
async fn assert_elapsed_values(pool: &mut db::DbPool) -> Result<(), db::DbError> {
    let moments = LiveMoment::table();
    let scope = || db::from(&moments).filter(moments.id.eq(db::val(1_i64)));
    let shifted = || {
        db::funcs::datetime::add(
            moments.happened_at.clone(),
            chrono::TimeDelta::milliseconds(1_500),
        )
    };
    let positive = scope()
        .scalar(db::funcs::datetime::diff_milliseconds(
            moments.happened_at.clone(),
            shifted(),
        ))
        .exec(&mut *pool)
        .await
        .map_err(|error| stage_error("positive diff_milliseconds", error))?;
    let negative = scope()
        .scalar(db::funcs::datetime::diff_milliseconds(
            shifted(),
            moments.happened_at.clone(),
        ))
        .exec(&mut *pool)
        .await
        .map_err(|error| stage_error("negative diff_milliseconds", error))?;

    assert_eq!(positive, 1_500);
    assert_eq!(negative, -1_500);
    Ok(())
}

/// Normalizes externally supplied SQLx test sessions before portable assertions.
async fn initialize_external_session(pool: &db::backend::Pool) -> Result<(), db::DbError> {
    #[cfg(feature = "postgres")]
    let sql = "SET TIME ZONE 'UTC'";
    #[cfg(any(feature = "mysql", feature = "mariadb"))]
    let sql = "SET time_zone = '+00:00'";
    #[cfg(feature = "sqlite")]
    let sql = "SELECT 1";

    db::sqlx::query(sql)
        .execute(pool)
        .await
        .map_err(|error| db::DbError::from_sqlx(db::DbOperation::Connect, error))?;
    Ok(())
}

#[cfg(feature = "time")]
async fn assert_time_decode(pool: &db::DbPool, expected: i64) -> Result<(), db::DbError> {
    let value: time::OffsetDateTime =
        db::sqlx::query_scalar("SELECT happened_at FROM mool_datetime_live WHERE id = 1")
            .fetch_one(pool.as_sqlx())
            .await
            .map_err(|error| db::DbError::from_sqlx(db::DbOperation::FetchScalar, error))?;
    assert_eq!(value.unix_timestamp(), expected);
    Ok(())
}

#[cfg(not(feature = "time"))]
async fn assert_time_decode(_pool: &db::DbPool, _expected: i64) -> Result<(), db::DbError> {
    Ok(())
}

fn statement(sql: &str) -> Result<db::Statement, db::QueryError> {
    db::query(sql).to_statement()
}

fn stage_error(operation: &'static str, error: db::DbError) -> db::DbError {
    db::DbError::Mock {
        operation,
        reason: error.to_string(),
    }
}

#[cfg(feature = "postgres")]
fn create_table_sql() -> &'static str {
    "CREATE TABLE mool_datetime_live (id BIGINT PRIMARY KEY, happened_at TIMESTAMPTZ NOT NULL)"
}

#[cfg(any(feature = "mysql", feature = "mariadb"))]
fn create_table_sql() -> &'static str {
    "CREATE TABLE mool_datetime_live (id BIGINT PRIMARY KEY, happened_at TIMESTAMP(6) NOT NULL)"
}

#[cfg(feature = "sqlite")]
fn create_table_sql() -> &'static str {
    "CREATE TABLE mool_datetime_live (id BIGINT PRIMARY KEY, happened_at TEXT NOT NULL)"
}

#[cfg(feature = "postgres")]
fn session_timezone_sql() -> &'static str {
    "SHOW TIME ZONE"
}

#[cfg(any(feature = "mysql", feature = "mariadb"))]
fn session_timezone_sql() -> &'static str {
    "SELECT @@session.time_zone"
}
