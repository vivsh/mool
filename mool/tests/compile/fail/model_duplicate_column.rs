use mool as db;

#[derive(Debug, Clone, db::Model)]
#[table(name = "duplicate_columns")]
struct DuplicateColumns {
    #[column(name = "value")]
    first: i64,
    #[column(name = "value")]
    second: i64,
}

fn main() {}
