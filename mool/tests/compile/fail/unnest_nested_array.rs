use mool as db;
use mool::Model;
use mool::backend::PostgresUnnestExt;

#[derive(Debug, Clone, db::Model)]
#[table(name = "matrix_rows")]
struct MatrixRow {
    id: i64,
    values: Vec<i64>,
}

fn main() {
    let table = MatrixRow::table();
    let rows = [MatrixRow {
        id: 1,
        values: vec![1, 2],
    }];
    let _ = db::from(&table)
        .batch_insert(&rows)
        .using_unnest()
        .plan();
}
