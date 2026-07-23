#![allow(dead_code)]

use mool as db;
use mool::Model;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, db::SqlEnum)]
#[sql_enum(rename_all = "snake_case")]
enum TextStatus {
    Draft,
    Published,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, db::SqlEnum)]
#[sql_enum(storage = "int", repr = "i16")]
enum IntStatus {
    #[sql_enum(code = 1)]
    Draft,
    #[sql_enum(code = 2)]
    Published,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, db::SqlEnum)]
#[sql_enum(storage = "native_postgres", name = "native_status")]
enum NativePostgresStatus {
    Draft,
    Published,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, db::SqlEnum)]
#[sql_enum(storage = "native_mysql", rename_all = "snake_case")]
enum NativeMysqlStatus {
    Draft,
    Published,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct JsonMeta {
    label: String,
}

#[derive(Debug, Clone, db::Model)]
#[table(name = "compat_rows")]
struct CompatRow {
    #[column(primary_key)]
    id: i64,
    #[column(type = "uuid")]
    external_id: uuid::Uuid,
    #[column(type = "timestamptz")]
    created_at: chrono::DateTime<chrono::Utc>,
    #[column(nullable)]
    subtitle: Option<String>,
    #[column(type = "jsonb")]
    meta: JsonMeta,
    #[column(sql_enum)]
    text_status: TextStatus,
    #[column(sql_enum)]
    int_status: IntStatus,
    #[column(sql_enum)]
    native_pg_status: NativePostgresStatus,
    #[column(sql_enum)]
    native_mysql_status: NativeMysqlStatus,
}

fn assert_sqlx_value<T>()
where
    T: Clone
        + for<'q> sqlx::Encode<'q, db::backend::Database>
        + for<'r> sqlx::Decode<'r, db::backend::Database>
        + sqlx::Type<db::backend::Database>
        + Send
        + Sync
        + Unpin
        + 'static,
{
}

#[cfg(feature = "postgres")]
fn assert_postgres_array<T>()
where
    T: sqlx::postgres::PgHasArrayType,
{
}

fn main() {
    assert_sqlx_value::<TextStatus>();
    assert_sqlx_value::<IntStatus>();
    assert_sqlx_value::<NativePostgresStatus>();
    assert_sqlx_value::<NativeMysqlStatus>();

    let rows = CompatRow::table();
    let _ = db::from(&rows)
        .filter(rows.text_status.eq(db::val(TextStatus::Published)))
        .filter(rows.int_status.eq(db::val(IntStatus::Draft)))
        .all::<CompatRow>()
        .plan()
        .unwrap();

    #[cfg(feature = "postgres")]
    {
        assert_postgres_array::<TextStatus>();
        assert_postgres_array::<IntStatus>();
        assert_postgres_array::<NativePostgresStatus>();
        assert_postgres_array::<NativeMysqlStatus>();
    }
}
