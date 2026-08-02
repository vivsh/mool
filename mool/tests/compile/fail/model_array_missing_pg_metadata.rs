#[derive(Debug, Clone, sqlx::Type)]
#[sqlx(transparent, no_pg_array)]
struct ScalarOnly(i64);

fn main() {
    let _ = mool::schema::__private::native_array_sql_type::<ScalarOnly>();
}
