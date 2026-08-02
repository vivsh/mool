//! Internal schema inference helpers used by Mool's derive macros.

use sqlx::{Type, TypeInfo};

use crate::backend::Database;

/// Returns the selected backend's native SQL array type for one element type.
///
/// PostgreSQL implements `Type` for `Vec<T>` when `T` has native array
/// metadata. Other backends reject non-byte vectors through the trait bound.
pub fn native_array_sql_type<T>() -> String
where
    Vec<T>: Type<Database>,
{
    let type_info = <Vec<T> as Type<Database>>::type_info();
    let name = type_info.name();
    postgres_array_ddl_name(name).to_owned()
}

/// Converts SQLx's PostgreSQL catalog aliases into Mool's stable DDL spelling.
fn postgres_array_ddl_name(name: &str) -> &str {
    match name {
        "BOOL[]" => "boolean[]",
        "INT2[]" => "smallint[]",
        "INT4[]" => "integer[]",
        "INT8[]" => "bigint[]",
        "FLOAT4[]" => "real[]",
        "FLOAT8[]" => "double precision[]",
        "TEXT[]" => "text[]",
        "BYTEA[]" => "bytea[]",
        "UUID[]" => "uuid[]",
        "DATE[]" => "date[]",
        "TIME[]" => "time[]",
        "TIMESTAMP[]" => "timestamp[]",
        "TIMESTAMPTZ[]" => "timestamptz[]",
        "JSON[]" => "json[]",
        "JSONB[]" => "jsonb[]",
        _ => name,
    }
}
